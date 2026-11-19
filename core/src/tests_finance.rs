//! Finance rebuild, phases 1 and 2 (docs/design/finance-standard.md): every posting rule and every report
//! checked against numbers worked out by hand. Money is AFN × 100, so 350_000 is 3 500 AFN.

use artaveo_shared::{methods as m, ErrorCode, RpcRequest, RpcResponse};
use serde_json::{json, Value};

use crate::tests::T;

fn failure(t: &T, method: &str, params: Value, token: &str) -> (ErrorCode, String, String) {
    match t.core.handle(RpcRequest { method: method.into(), params, token: Some(token.into()) }) {
        RpcResponse::Ok { result } => panic!("{method} unexpectedly succeeded: {result}"),
        RpcResponse::Error { error } => (
            error.code,
            error.field.clone().unwrap_or_default(),
            error
                .rule
                .map(|r| serde_json::to_value(r).unwrap().as_str().unwrap().to_string())
                .unwrap_or_default(),
        ),
    }
}

/// (field, rule) of a call that must fail.
fn fr(t: &T, method: &str, params: Value, token: &str) -> (String, String) {
    let (_, f, r) = failure(t, method, params, token);
    (f, r)
}

fn day(n: i64) -> String {
    let d = crate::scheduling::parse_date("d", &crate::clock::today_iso()).unwrap() + time::Duration::days(n);
    crate::scheduling::format_date(d)
}

fn i(v: &Value) -> i64 {
    v.as_i64().unwrap_or_else(|| panic!("not a number: {v}"))
}

struct Fx {
    t: T,
    owner: String,
    doctor: String,
    patient: String,
    patient2: String,
}

fn fx() -> Fx {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let d = t.ok(m::DOCTORS_CREATE, json!({"full_name": "دکتر احمد", "color": "#0E7490"}), Some(&owner));
    let p = t.ok(m::PATIENTS_CREATE, json!({"full_name": "زهرا", "phone": "0700111111"}), Some(&owner));
    let q = t.ok(m::PATIENTS_CREATE, json!({"full_name": "حمید", "phone": "0700222222"}), Some(&owner));
    Fx {
        doctor: d["id"].as_str().unwrap().to_string(),
        patient: p["id"].as_str().unwrap().to_string(),
        patient2: q["id"].as_str().unwrap().to_string(),
        owner,
        t,
    }
}

impl Fx {
    fn ok(&self, method: &str, params: Value) -> Value {
        self.t.ok(method, params, Some(&self.owner))
    }
    fn fr(&self, method: &str, params: Value) -> (String, String) {
        fr(&self.t, method, params, &self.owner)
    }
    fn service(&self, code: &str) -> String {
        self.ok(m::CATALOG_GET, json!({}))["services"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["code"] == code)
            .unwrap_or_else(|| panic!("service {code}"))["id"]
            .as_str()
            .unwrap()
            .to_string()
    }
    /// A treatment done today at the given price.
    fn done(&self, patient: &str, tooth: u8, price: i64) -> String {
        let item = self.ok(
            m::TREATMENT_ITEMS_ADD,
            json!({"patient_id": patient, "service_id": self.service("SRG-01"), "teeth": [tooth], "perform_now": true,
                   "doctor_id": self.doctor, "unit_price": price}),
        );
        item[0]["id"].as_str().unwrap().to_string()
    }
    fn invoice(&self, id: &Value) -> Value {
        self.ok(m::FIN_INVOICES_GET, json!({"id": id}))
    }
    fn account(&self, patient: &str) -> Value {
        self.ok(m::FIN_PATIENTS_ACCOUNT, json!({"patient_id": patient}))
    }
    fn balance(&self, account: i64) -> i64 {
        let all = self.ok(m::FIN_ACCOUNTS_LIST, json!({"include_inactive": true}));
        i(&all.as_array().unwrap().iter().find(|a| a["id"] == account).unwrap()["balance"])
    }
    fn supplier(&self, name: &str) -> String {
        let s = self.ok(m::SUPPLIERS_SAVE, json!({"kind": "supplier", "name": name, "tin": "1000123456"}));
        s["id"].as_str().unwrap().to_string()
    }
    fn report(&self, kind: &str) -> Value {
        let today = crate::clock::today_iso();
        self.ok(m::REPORTS_RUN, json!({"kind": kind, "from": today, "to": today}))
    }
    /// The ledger rows of a document: (account, amount).
    fn rows(&self, doc: &Value) -> Vec<(i64, i64)> {
        let id = doc.as_i64().unwrap();
        self.t
            .core
            .with_db(|o| {
                let mut st = o.conn.prepare(
                    "SELECT account_id, amount FROM fin_gl WHERE doc_id = ?1 AND rev = 0 ORDER BY no",
                )?;
                let r = st
                    .query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                Ok(r)
            })
            .unwrap()
    }
}

#[test]
fn patient_money_posts_by_the_rules_and_the_books_reconcile() {
    let f = fx();
    let t1 = f.done(&f.patient, 36, 350_000);
    let t2 = f.done(&f.patient, 48, 80_000);
    assert_eq!(f.ok(m::FIN_INVOICES_BILLABLE, json!({"patient_id": f.patient})).as_array().unwrap().len(), 2);

    // Refused: no lines; a line discount above its line; a whole discount above what is left.
    assert_eq!(
        f.fr(m::FIN_INVOICES_CREATE, json!({"patient_id": f.patient})),
        ("items".into(), "invoice_empty".into())
    );
    assert_eq!(
        f.fr(
            m::FIN_INVOICES_CREATE,
            json!({"patient_id": f.patient, "items": [{"item_id": t2, "discount": 90_000}]})
        ),
        ("discount".into(), "discount_range".into())
    );

    // INV: 350 000 + 80 000 (line discount 10 000), invoice discount 30 000 shared 25 000 / 5 000.
    let inv = f.ok(
        m::FIN_INVOICES_CREATE,
        json!({"patient_id": f.patient, "items": [{"item_id": t1}, {"item_id": t2, "discount": 10_000}], "discount": 30_000}),
    );
    assert_eq!((i(&inv["gross"]), i(&inv["discount"]), i(&inv["net"])), (430_000, 40_000, 390_000));
    assert_eq!(inv["number"].as_str().unwrap().len(), "INV-1405-000001".len());
    assert!(inv["number"].as_str().unwrap().starts_with("INV-"));
    let lines = inv["lines"].as_array().unwrap();
    assert_eq!((i(&lines[0]["discount"]), i(&lines[1]["discount"])), (25_000, 15_000));
    assert_eq!(lines[0]["doctor_id"], json!(f.doctor));
    // Dr receivable 390 000, Dr discounts 40 000, Cr income 430 000.
    assert_eq!(f.rows(&inv["id"]), vec![(1310, 390_000), (4910, 40_000), (4110, -430_000)]);
    assert!(f.ok(m::FIN_INVOICES_BILLABLE, json!({"patient_id": f.patient})).as_array().unwrap().is_empty());

    // RC1: 150 000 cash, put on the oldest invoice.
    let rc1 = f.ok(
        m::FIN_RECEIPTS_CREATE,
        json!({"patient_id": f.patient, "account_id": 1110, "amount": 150_000, "auto_allocate": true}),
    );
    assert_eq!((i(&rc1["to_deposit"]), i(&rc1["owed_after"]), i(&rc1["deposit_after"])), (0, 240_000, 0));
    assert_eq!(f.rows(&rc1["id"]), vec![(1110, 150_000), (1310, -150_000)]);
    // More than the invoice owes is refused; an allocation above the money too.
    assert_eq!(
        f.fr(
            m::FIN_RECEIPTS_CREATE,
            json!({"patient_id": f.patient, "account_id": 1210, "amount": 300_000,
                   "allocations": [{"invoice_id": inv["id"], "amount": 250_000}]})
        ),
        ("allocations".into(), "payment_exceeds".into())
    );
    assert_eq!(
        f.fr(m::FIN_RECEIPTS_CREATE, json!({"patient_id": f.patient, "account_id": 4110, "amount": 1})).1,
        "account_kind"
    );

    // RC2: 300 000 by bank, 200 000 on the invoice, 100 000 kept as deposit.
    let rc2 = f.ok(
        m::FIN_RECEIPTS_CREATE,
        json!({"patient_id": f.patient, "account_id": 1210, "amount": 300_000,
               "allocations": [{"invoice_id": inv["id"], "amount": 200_000}]}),
    );
    assert_eq!(i(&rc2["to_deposit"]), 100_000);
    assert_eq!(f.rows(&rc2["id"]), vec![(1210, 300_000), (1310, -200_000), (2210, -100_000)]);

    // Plan for the 40 000 left: two installments of 20 000; it must add up.
    assert_eq!(
        f.fr(
            m::FIN_INVOICES_SET_PLAN,
            json!({"invoice_id": inv["id"], "installments": [{"due_date": day(10), "amount": 30_000}]})
        ),
        ("installments".into(), "installments_invalid".into())
    );
    let with_plan = f.ok(
        m::FIN_INVOICES_SET_PLAN,
        json!({"invoice_id": inv["id"], "installments": [{"due_date": day(10), "amount": 20_000},
                                                          {"due_date": day(40), "amount": 20_000}]}),
    );
    assert_eq!(with_plan["plan"]["installments"].as_array().unwrap().len(), 2);
    // Replacing a plan needs a reason.
    assert_eq!(
        f.fr(
            m::FIN_INVOICES_SET_PLAN,
            json!({"invoice_id": inv["id"], "installments": [{"due_date": day(5), "amount": 40_000}]})
        ),
        ("reason".into(), "required".into())
    );

    // CN 10 000: the last installment shrinks to 10 000.
    let after_cn = f.ok(
        m::FIN_INVOICES_ADJUST,
        json!({"invoice_id": inv["id"], "kind": "reduction", "amount": 10_000, "reason": "goodwill"}),
    );
    assert_eq!(i(&after_cn["remaining"]), 30_000);
    let plan = after_cn["plan"]["installments"].as_array().unwrap().clone();
    assert_eq!((i(&plan[0]["amount"]), i(&plan[1]["amount"])), (20_000, 10_000));
    assert_eq!(
        f.fr(
            m::FIN_INVOICES_ADJUST,
            json!({"invoice_id": inv["id"], "kind": "reduction", "amount": 40_000, "reason": "x"})
        ),
        ("amount".into(), "credit_note_exceeds".into())
    );

    // AL: 30 000 of the deposit settles the invoice and both installments.
    let al = f.ok(
        m::FIN_RECEIPTS_ALLOCATE,
        json!({"patient_id": f.patient, "allocations": [{"invoice_id": inv["id"], "amount": 30_000}]}),
    );
    assert_eq!(f.rows(&al["id"]), vec![(2210, 30_000), (1310, -30_000)]);
    let paid = f.invoice(&inv["id"]);
    assert_eq!((i(&paid["remaining"]), paid["pay_state"].as_str()), (0, Some("paid")));
    assert!(paid["plan"]["installments"].as_array().unwrap().iter().all(|x| x["state"] == "paid"));

    // RF: 50 000 of the deposit back in cash; more than the deposit is refused.
    assert_eq!(
        f.fr(
            m::FIN_RECEIPTS_REFUND,
            json!({"patient_id": f.patient, "account_id": 1110, "amount": 80_000, "reason": "x"})
        ),
        ("amount".into(), "insufficient_credit".into())
    );
    let rf = f.ok(
        m::FIN_RECEIPTS_REFUND,
        json!({"patient_id": f.patient, "account_id": 1110, "amount": 50_000, "reason": "moved away"}),
    );
    assert_eq!(f.rows(&rf["id"]), vec![(2210, 50_000), (1110, -50_000)]);
    // Refund of 15 000 paid on the invoice: taken off the invoice back to the deposit, then paid out.
    f.ok(
        m::FIN_RECEIPTS_REFUND,
        json!({"patient_id": f.patient, "account_id": 1110, "amount": 15_000, "reason": "overcharged", "from_invoice_id": inv["id"]}),
    );
    assert_eq!(i(&f.invoice(&inv["id"])["remaining"]), 15_000);
    // WO: the 15 000 is written off.
    let wo = f.ok(
        m::FIN_INVOICES_ADJUST,
        json!({"invoice_id": inv["id"], "kind": "write_off", "amount": 15_000, "reason": "uncollectable"}),
    );
    assert_eq!((i(&wo["remaining"]), i(&wo["written_off"]), i(&wo["allocated"])), (0, 15_000, 365_000));

    let acc = f.account(&f.patient);
    assert_eq!((i(&acc["owed"]), i(&acc["deposit"]), i(&acc["balance"])), (0, 20_000, -20_000));
    assert_eq!(
        (i(&acc["invoiced"]), i(&acc["reductions"]), i(&acc["written_off"])),
        (390_000, 10_000, 15_000)
    );
    // Ledger balances: cash 150 000 − 50 000 − 15 000, bank 300 000, receivable 0, deposits 20 000.
    assert_eq!(
        (f.balance(1110), f.balance(1210), f.balance(1310), f.balance(2210)),
        (85_000, 300_000, 0, -20_000)
    );
    assert_eq!(
        (f.balance(4110), f.balance(4910), f.balance(4920), f.balance(5810)),
        (-430_000, 40_000, 10_000, 15_000)
    );

    // The statement: one invoice 390 000, credits 150 000 + 300 000 − 50 000 − 15 000 + 10 000 + 15 000.
    let st = f.ok(m::FIN_PATIENTS_STATEMENT, json!({"patient_id": f.patient}));
    assert_eq!((i(&st["charges"]), i(&st["credits"]), i(&st["closing"])), (390_000, 410_000, -20_000));

    // The overview of today proves the books agree with themselves.
    let today = crate::clock::today_iso();
    let o = f.ok(m::FIN_OVERVIEW, json!({"from": today, "to": today}));
    assert!(o["checks"].as_array().unwrap().iter().all(|c| c["ok"] == true), "{}", o["checks"]);
    assert_eq!(
        (i(&o["cash"]["received"]), i(&o["cash"]["refunded"]), i(&o["cash"]["net_received"])),
        (450_000, 65_000, 385_000)
    );
    assert_eq!(
        (i(&o["accrual"]["revenue"]), i(&o["accrual"]["write_offs"]), i(&o["accrual"]["result"])),
        (380_000, 15_000, 365_000)
    );
    assert_eq!((i(&o["receivables"]), i(&o["deposits"])), (0, 20_000));
}

#[test]
fn money_out_bills_transfers_and_cash_counts_post_by_the_rules() {
    let f = fx();
    let today = crate::clock::today_iso();
    // Money in first: 450 000 cash, 300 000 bank from a patient deposit.
    f.ok(m::FIN_RECEIPTS_CREATE, json!({"patient_id": f.patient, "account_id": 1110, "amount": 450_000}));
    f.ok(m::FIN_RECEIPTS_CREATE, json!({"patient_id": f.patient, "account_id": 1210, "amount": 300_000}));

    // Rent 200 000 paid by bank, 20 000 rent tax kept back (art. 59, entered by hand).
    let rent = f.ok(
        m::FIN_VOUCHERS_CREATE,
        json!({"date": today, "account_id": 1210, "purpose": "expense", "name": "صاحب ملک",
               "lines": [{"account_id": 5210, "amount": 200_000, "withheld": 20_000, "withholding": "rent"}]}),
    );
    assert_eq!((i(&rent["gross"]), i(&rent["withheld"]), i(&rent["net"])), (200_000, 20_000, 180_000));
    assert_eq!(f.rows(&rent["id"]), vec![(5210, 200_000), (2420, -20_000), (1210, -180_000)]);
    // Withheld tax needs its kind; an expense line must be an expense account.
    assert_eq!(
        f.fr(
            m::FIN_VOUCHERS_CREATE,
            json!({"date": today, "account_id": 1210, "purpose": "expense", "lines": [{"account_id": 5210, "amount": 1000, "withheld": 10}]})
        ),
        ("withholding".into(), "required".into())
    );
    assert_eq!(
        f.fr(
            m::FIN_VOUCHERS_CREATE,
            json!({"date": today, "account_id": 1210, "purpose": "expense", "lines": [{"account_id": 1310, "amount": 1000}]})
        )
        .1,
        "account_kind"
    );
    // A voucher cannot be dated in the future.
    assert_eq!(
        f.fr(
            m::FIN_VOUCHERS_CREATE,
            json!({"date": day(2), "account_id": 1210, "purpose": "expense", "lines": [{"account_id": 5210, "amount": 1000}]})
        ),
        ("date".into(), "date_format".into())
    );

    // A supplier bill for medicines (60 000), then 40 000 paid to the supplier from cash.
    let s = f.supplier("دواخانه صحت");
    let bill = f.ok(
        m::FIN_BILLS_CREATE,
        json!({"supplier_id": s, "date": today, "reference": "B-77", "lines": [{"account_id": 5260, "amount": 60_000}]}),
    );
    assert_eq!(f.rows(&bill["id"]), vec![(5260, 60_000), (2110, -60_000)]);
    let pay = f.ok(
        m::FIN_VOUCHERS_CREATE,
        json!({"date": today, "account_id": 1110, "purpose": "supplier", "supplier_id": s, "amount": 40_000}),
    );
    assert_eq!(f.rows(&pay["id"]), vec![(2110, 40_000), (1110, -40_000)]);
    let sup = f.ok(m::SUPPLIERS_GET, json!({"id": s}));
    assert_eq!(
        (i(&sup["purchases_total"]), i(&sup["paid_total"]), i(&sup["balance"])),
        (60_000, 40_000, 20_000)
    );
    assert_eq!(sup["tin"], json!("1000123456"));
    let ledger = f.ok(m::SUPPLIER_LEDGER_LIST, json!({"supplier_id": s}));
    let last = ledger.as_array().unwrap().last().unwrap().clone();
    assert_eq!((last["kind"].as_str(), i(&last["balance"])), (Some("payment"), 20_000));

    // The rent tax is paid to the government (20 000 by bank).
    f.ok(
        m::FIN_VOUCHERS_CREATE,
        json!({"date": today, "account_id": 1210, "purpose": "tax", "withholding": "rent", "amount": 20_000}),
    );
    // The owner puts 50 000 in; 30 000 cash goes to the bank.
    let cap = f.ok(
        m::FIN_OTHER_RECEIPTS_CREATE,
        json!({"date": today, "account_id": 1110, "source": "capital", "amount": 50_000}),
    );
    assert_eq!(f.rows(&cap["id"]), vec![(1110, 50_000), (3110, -50_000)]);
    assert_eq!(
        f.fr(
            m::FIN_TRANSFERS_CREATE,
            json!({"date": today, "from_account_id": 1110, "to_account_id": 1110, "amount": 1})
        )
        .1,
        "same_account"
    );
    f.ok(
        m::FIN_TRANSFERS_CREATE,
        json!({"date": today, "from_account_id": 1110, "to_account_id": 1210, "amount": 30_000}),
    );

    // Cash: 450 000 − 40 000 + 50 000 − 30 000 = 430 000; bank: 300 000 − 180 000 − 20 000 + 30 000 = 130 000.
    let cash = f.ok(m::FIN_CASH_DAY, json!({"account_id": 1110, "date": today}));
    assert_eq!(
        (
            i(&cash["opening"]),
            i(&cash["received"]),
            i(&cash["paid_out"]),
            i(&cash["other_in"]),
            i(&cash["transfers"]),
            i(&cash["closing"])
        ),
        (0, 450_000, 40_000, 50_000, -30_000, 430_000)
    );
    assert_eq!(f.balance(1210), 130_000);
    // A count 1 000 short needs a reason; it is booked to over/short.
    assert_eq!(
        f.fr(m::FIN_CASH_COUNT, json!({"account_id": 1110, "date": today, "counted": 429_000})),
        ("reason".into(), "cash_reason_required".into())
    );
    let cc = f.ok(
        m::FIN_CASH_COUNT,
        json!({"account_id": 1110, "date": today, "counted": 429_000, "reason": "short"}),
    );
    assert_eq!((i(&cc["book"]), i(&cc["difference"])), (430_000, -1_000));
    assert_eq!(f.balance(1110), 429_000);
    assert_eq!(f.balance(5910), 1_000);

    let o = f.ok(m::FIN_OVERVIEW, json!({"from": today, "to": today}));
    assert!(o["checks"].as_array().unwrap().iter().all(|c| c["ok"] == true), "{}", o["checks"]);
    let c = &o["cash"];
    assert_eq!(
        (
            i(&c["net_received"]),
            i(&c["expenses_total"]),
            i(&c["suppliers_paid"]),
            i(&c["withheld"]),
            i(&c["tax_paid"])
        ),
        (750_000, 200_000, 40_000, 20_000, 20_000)
    );
    assert_eq!(
        (i(&c["owner_in"]), i(&c["count_differences"]), i(&c["money_closing"])),
        (50_000, -1_000, 559_000)
    );
    // Accrual: costs are rent 200 000 + medicines 60 000 + cash short 1 000.
    assert_eq!(
        (i(&o["accrual"]["expenses_total"]), i(&o["payables"]), i(&o["tax_withheld"])),
        (261_000, 20_000, 0)
    );

    // Voiding the supplier payment reverses it; the supplier is owed 60 000 again.
    f.ok(m::FIN_DOCS_VOID, json!({"id": pay["id"], "reason": "wrong supplier"}));
    assert_eq!(i(&f.ok(m::SUPPLIERS_GET, json!({"id": s}))["balance"]), 60_000);
    assert_eq!(
        f.fr(m::FIN_DOCS_VOID, json!({"id": pay["id"], "reason": "again"})),
        ("id".into(), "doc_void".into())
    );
}

#[test]
fn voids_reverse_and_never_break_later_documents() {
    let f = fx();
    let q = f.patient2.clone();
    let inv = f.ok(
        m::FIN_INVOICES_CREATE,
        json!({"patient_id": q, "extra_lines": [{"description": "عکس", "quantity": 1, "unit_price": 100_000}]}),
    );
    let rc = f.ok(
        m::FIN_RECEIPTS_CREATE,
        json!({"patient_id": q, "account_id": 1110, "amount": 100_000, "auto_allocate": true}),
    );
    // An invoice with money on it cannot be voided; void the receipt first.
    assert_eq!(
        f.fr(m::FIN_DOCS_VOID, json!({"id": inv["id"], "reason": "x"})),
        ("id".into(), "invoice_has_payments".into())
    );
    f.ok(m::FIN_DOCS_VOID, json!({"id": rc["id"], "reason": "typed twice"}));
    assert_eq!(i(&f.invoice(&inv["id"])["remaining"]), 100_000);
    let void = f.ok(m::FIN_DOCS_VOID, json!({"id": inv["id"], "reason": "wrong patient"}));
    assert_eq!(void["status"], json!("void"));
    // The posting is reversed (rev rows), never deleted.
    let net: i64 =
        f.t.core
            .with_db(|o| {
                Ok(o.conn.query_row(
                    "SELECT SUM(amount), COUNT(*) FROM fin_gl WHERE doc_id = ?1",
                    [i(&inv["id"])],
                    |r| Ok(r.get::<_, i64>(0)? * 100 + r.get::<_, i64>(1)?),
                )?)
            })
            .unwrap();
    assert_eq!(net, 4, "2 rows posted (receivable, income) + 2 reversed, adding up to 0");

    // A deposit already put on an invoice cannot vanish by voiding its receipt.
    let rc2 = f.ok(m::FIN_RECEIPTS_CREATE, json!({"patient_id": q, "account_id": 1110, "amount": 50_000}));
    let inv2 = f.ok(
        m::FIN_INVOICES_CREATE,
        json!({"patient_id": q, "use_deposit": true, "extra_lines": [{"description": "معاینه", "quantity": 1, "unit_price": 50_000}]}),
    );
    assert_eq!(i(&inv2["remaining"]), 0);
    assert_eq!(
        f.fr(m::FIN_DOCS_VOID, json!({"id": rc2["id"], "reason": "x"})),
        ("id".into(), "doc_not_voidable".into())
    );

    // Issued rows are never rewritten, even by hand.
    let blocked = f.t.core.with_db(|o| {
        Ok(o.conn.execute("UPDATE fin_gl SET amount = 1 WHERE doc_id = ?1", [i(&rc2["id"])])?)
    });
    assert!(blocked.is_err());
    let blocked =
        f.t.core.with_db(|o| Ok(o.conn.execute("DELETE FROM fin_doc WHERE id = ?1", [i(&rc2["id"])])?));
    assert!(blocked.is_err());

    let today = crate::clock::today_iso();
    let o = f.ok(m::FIN_OVERVIEW, json!({"from": today, "to": today}));
    assert!(o["checks"].as_array().unwrap().iter().all(|c| c["ok"] == true), "{}", o["checks"]);
}

#[test]
fn purchases_and_lab_cases_become_supplier_bills() {
    let f = fx();
    let today = crate::clock::today_iso();
    let s = f.supplier("شرکت مواد");
    let item = f.ok(
        m::STOCK_ITEMS_SAVE,
        json!({"name": {"fa": "کامپوزیت", "ps": "کامپوزیت", "en": "Composite"}, "category_id": "sc-restorative", "unit_id": "su-tube"}),
    );
    // 2 tubes at 15 000, 10 000 paid on the spot from cash.
    let pu = f.ok(
        m::PURCHASES_CREATE,
        json!({"supplier_id": s, "date": today, "paid": 10_000, "account_id": 1110,
               "lines": [{"item_id": item["id"], "qty": 200, "unit_cost": 15_000, "location_id": "sl-main"}]}),
    );
    let sup = f.ok(m::SUPPLIERS_GET, json!({"id": s}));
    assert_eq!(
        (i(&sup["purchases_total"]), i(&sup["paid_total"]), i(&sup["balance"])),
        (30_000, 10_000, 20_000)
    );
    assert_eq!(f.balance(5230), 30_000);
    // The bill of a purchase is voided with the purchase, not on its own.
    let bills = f.ok(m::FIN_VOUCHERS_LIST, json!({"kind": "bl", "supplier_id": s}));
    let bill_id = bills[0]["id"].clone();
    assert_eq!(f.fr(m::FIN_DOCS_VOID, json!({"id": bill_id, "reason": "x"})).1, "doc_not_voidable");
    f.ok(m::PURCHASES_VOID, json!({"id": pu["id"], "version": pu["version"], "reason": "wrong items"}));
    let sup = f.ok(m::SUPPLIERS_GET, json!({"id": s}));
    assert_eq!((i(&sup["purchases_total"]), i(&sup["balance"])), (0, -10_000));

    // A lab case is a bill of the lab once it comes back.
    let lab = f.ok(m::SUPPLIERS_SAVE, json!({"kind": "lab", "name": "لابراتوار نور"}));
    let case = f.ok(
        m::LAB_CASES_CREATE,
        json!({"patient_id": f.patient, "lab_id": lab["id"], "work_type": "crown", "teeth": [11], "cost": 120_000}),
    );
    assert_eq!(i(&f.ok(m::SUPPLIERS_GET, json!({"id": lab["id"]}))["balance"]), 0);
    let sent = f
        .ok(m::LAB_CASES_SET_STATUS, json!({"id": case["id"], "version": case["version"], "status": "sent"}));
    f.ok(
        m::LAB_CASES_SET_STATUS,
        json!({"id": case["id"], "version": sent["version"], "status": "received"}),
    );
    let l = f.ok(m::SUPPLIERS_GET, json!({"id": lab["id"]}));
    assert_eq!((i(&l["lab_total"]), i(&l["balance"])), (120_000, 120_000));
    assert_eq!(f.balance(5240), 120_000);
}

#[test]
fn reports_add_up_to_the_ledger() {
    let f = fx();
    let t1 = f.done(&f.patient, 36, 350_000);
    let inv = f.ok(
        m::FIN_INVOICES_CREATE,
        json!({"patient_id": f.patient, "items": [{"item_id": t1}], "discount": 50_000}),
    );
    f.ok(
        m::FIN_RECEIPTS_CREATE,
        json!({"patient_id": f.patient, "account_id": 1110, "amount": 200_000, "auto_allocate": true}),
    );
    f.ok(m::FIN_RECEIPTS_REFUND, json!({"patient_id": f.patient, "account_id": 1110, "amount": 20_000, "reason": "r", "from_invoice_id": inv["id"]}));
    let today = crate::clock::today_iso();
    f.ok(
        m::FIN_VOUCHERS_CREATE,
        json!({"date": today, "account_id": 1110, "purpose": "expense", "lines": [{"account_id": 5220, "amount": 15_000, "description": "برق"}]}),
    );

    // Daily summary: gross 350 000, discount 50 000, invoiced 300 000, received 200 000, refunded 20 000.
    let d = f.report("daily_summary");
    let tot = d["totals"].as_array().unwrap();
    let v = |c: &Value| i(&c["value"]);
    assert_eq!(
        (v(&tot[2]), v(&tot[3]), v(&tot[4]), v(&tot[6]), v(&tot[7]), v(&tot[8])),
        (350_000, 50_000, 300_000, 200_000, 20_000, 180_000)
    );
    // Receipts register: one receipt, one refund.
    let r = f.report("receipts");
    assert_eq!(r["rows"].as_array().unwrap().len(), 2);
    assert_eq!((v(&r["totals"][5]), v(&r["totals"][6])), (200_000, 20_000));
    // Invoices register: remaining = 300 000 − 200 000 + 20 000.
    assert_eq!(v(&f.report("invoices")["totals"][9]), 120_000);
    // Aging: all 120 000 in the first bucket; adjustments: the discount and the money taken back.
    let a = f.report("aging");
    assert_eq!((v(&a["totals"][4]), v(&a["totals"][8])), (120_000, 120_000));
    assert_eq!(f.report("adjustments")["rows"].as_array().unwrap().len(), 3);
    // Expenses register and the cash book: 200 000 − 20 000 − 15 000 = 165 000.
    assert_eq!(v(&f.report("expenses")["totals"][7]), 15_000);
    let book = f.report("money_book");
    let last = book["rows"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(v(&last[6]), 165_000);
    // Income statement: cash result 180 000 − 15 000; accrual result 300 000 − 15 000.
    let is = f.report("income_statement");
    assert_eq!((v(&is["totals"][3]), v(&is["totals"][6])), (165_000, 285_000));
    for kind in [
        "monthly_summary",
        "installments",
        "deposits",
        "provider_production",
        "service_revenue",
        "doctor_shares",
        "cash_counts",
    ] {
        f.report(kind);
    }
}

#[test]
fn accounts_settings_and_permissions() {
    let f = fx();
    // A clinic's own expense account and a second bank account get the next free codes.
    let fuel = f.ok(m::FIN_ACCOUNTS_SAVE, json!({"parent_id": 5000, "name": "تیل جنراتور"}));
    assert_eq!(fuel["id"], json!(5300));
    let bank = f.ok(
        m::FIN_ACCOUNTS_SAVE,
        json!({"parent_id": 1100, "name": "عزیزی بانک", "money": "bank", "bank_no": "123"}),
    );
    assert_eq!(bank["id"], json!(1211));
    assert_eq!(f.fr(m::FIN_ACCOUNTS_SAVE, json!({"parent_id": 5000})).1, "account_name");
    assert_eq!(f.fr(m::FIN_ACCOUNTS_SAVE, json!({"id": 1110, "is_active": false})).1, "account_kind");

    let mut s = f.ok(m::FIN_SETTINGS_GET, json!({}));
    s["legal"]["tin"] = json!("1001234567");
    s["legal"]["legal_form"] = json!("natural_person");
    s["receipt_footer"] = json!("تشکر");
    let saved = f.ok(m::FIN_SETTINGS_SAVE, s.clone());
    assert_eq!(
        (saved["legal"]["tin"].as_str(), saved["receipt_footer"].as_str()),
        (Some("1001234567"), Some("تشکر"))
    );
    s["legal"]["tin"] = json!("12 34");
    assert_eq!(f.fr(m::FIN_SETTINGS_SAVE, s).0, "tin");

    // An assistant sees no money; a receptionist takes money but cannot refund or void.
    f.ok(
        m::USERS_CREATE,
        json!({"username": "rec", "display_name": "پذیرش", "password": "rec-pass-123", "role": "receptionist"}),
    );
    let rec = f.t.login("rec", "rec-pass-123");
    let r = f.t.call(
        m::FIN_RECEIPTS_CREATE,
        json!({"patient_id": f.patient, "account_id": 1110, "amount": 1_000}),
        Some(&rec),
    );
    assert!(r.is_ok());
    let refund = f.t.call(
        m::FIN_RECEIPTS_REFUND,
        json!({"patient_id": f.patient, "account_id": 1110, "amount": 1_000, "reason": "x"}),
        Some(&rec),
    );
    assert_eq!(refund.unwrap_err(), ErrorCode::Forbidden);
    assert_eq!(
        f.t.call(m::FIN_OVERVIEW, json!({"from": "2026-01-01", "to": "2026-01-02"}), Some(&rec)).unwrap_err(),
        ErrorCode::Forbidden
    );
}

#[test]
fn a_merged_patient_brings_the_money_along() {
    let f = fx();
    f.ok(m::FIN_RECEIPTS_CREATE, json!({"patient_id": f.patient2, "account_id": 1110, "amount": 70_000}));
    let q = f.ok(m::PATIENTS_GET, json!({"patient_id": f.patient2}));
    f.ok(
        m::PATIENTS_MERGE,
        json!({"keep_id": f.patient, "merge_id": f.patient2, "merge_id_version": q["version"]}),
    );
    assert_eq!(i(&f.account(&f.patient)["deposit"]), 70_000);
    // A patient with money is archived, never deleted.
    let p = f.ok(m::PATIENTS_GET, json!({"patient_id": f.patient}));
    assert_eq!(
        f.fr(m::PATIENTS_DELETE, json!({"id": f.patient, "version": p["version"]})).1,
        "patient_has_history"
    );
}

// ───────────────────────────── phase 2: staff, payroll, share statements ─────────────────────────────

impl Fx {
    fn employee(&self, name: &str, base: i64) -> String {
        let e = self.ok(
            m::FIN_EMPLOYEES_SAVE,
            json!({"full_name": name, "position": "نرس", "hire_date": day(-60), "base_salary": base, "tin": "100200300"}),
        );
        e["id"].as_str().unwrap().to_string()
    }
    fn employee_info(&self, id: &str) -> Value {
        let all = self.ok(m::FIN_EMPLOYEES_LIST, json!({"include_inactive": true}));
        all.as_array().unwrap().iter().find(|e| e["id"] == id).unwrap().clone()
    }
}

#[test]
fn payroll_books_salaries_tax_and_advances_and_only_what_is_owed_is_paid() {
    let f = fx();
    let today = crate::clock::today_iso();
    // Money in: owner capital 600 000 in cash, 300 000 in the bank.
    f.ok(m::FIN_OTHER_RECEIPTS_CREATE, json!({"date": today, "account_id": 1110, "source": "capital", "amount": 600_000}));
    f.ok(m::FIN_OTHER_RECEIPTS_CREATE, json!({"date": today, "account_id": 1210, "source": "capital", "amount": 300_000}));

    assert_eq!(
        f.fr(m::FIN_EMPLOYEES_SAVE, json!({"full_name": " ", "position": "x", "hire_date": today, "base_salary": 0})),
        ("full_name".into(), "required".into())
    );
    assert_eq!(
        f.fr(
            m::FIN_EMPLOYEES_SAVE,
            json!({"full_name": "x", "position": "x", "hire_date": today, "end_date": day(-1), "base_salary": 0})
        ),
        ("end_date".into(), "period_range".into())
    );
    let e1 = f.employee("کریم", 200_000);
    let e2 = f.employee("نسرین", 150_000);
    let e3 = f.employee("جاوید", 80_000);

    // Advances: E1 50 000 and E3 20 000 from the cash box (Dr 1320 for the employee, Cr cash).
    let adv1 = f.ok(
        m::FIN_VOUCHERS_CREATE,
        json!({"date": today, "account_id": 1110, "purpose": "advance", "employee_id": e1, "amount": 50_000}),
    );
    assert_eq!(f.rows(&adv1["id"]), vec![(1320, 50_000), (1110, -50_000)]);
    assert_eq!(adv1["employee_id"], json!(e1));
    assert_eq!(adv1["party_name"], json!("کریم"));
    let adv3 = f.ok(
        m::FIN_VOUCHERS_CREATE,
        json!({"date": today, "account_id": 1110, "purpose": "advance", "employee_id": e3, "amount": 20_000}),
    );
    assert_eq!(
        f.fr(m::FIN_VOUCHERS_CREATE, json!({"date": today, "account_id": 1110, "purpose": "advance", "amount": 1}))
            .0,
        "employee_id"
    );
    assert_eq!(i(&f.employee_info(&e1)["advances"]), 50_000);

    // The draft: every employee at base salary, with what they still owe in advances.
    let (from, to) = (day(-30), today.clone());
    let draft = f.ok(m::FIN_PAYROLL_DRAFT, json!({"from": from, "to": to}));
    assert!(draft["id"].is_null());
    assert_eq!(draft["lines"].as_array().unwrap().len(), 3);
    assert_eq!(i(&draft["gross"]), 430_000);
    let line = |emp: &str, base: i64, extra: i64, commission: i64, advance: i64, tax: i64| {
        json!({"employee_id": emp, "base": base, "extra": extra, "commission": commission, "advance": advance, "tax": tax})
    };
    let pay = |lines: Value| json!({"from": from, "to": to, "date": today, "lines": lines});
    // More advance taken back than was given; more deducted than earned.
    assert_eq!(
        f.fr(m::FIN_PAYROLL_CREATE, pay(json!([line(&e1, 200_000, 0, 0, 60_000, 0)]))),
        ("lines.0.advance".into(), "advance_exceeds".into())
    );
    assert_eq!(
        f.fr(m::FIN_PAYROLL_CREATE, pay(json!([line(&e1, 1, 0, 0, 0, 0), line(&e2, 150_000, 0, 0, 0, 160_000)]))),
        ("lines.1.tax".into(), "withheld_range".into())
    );

    // PR: E1 200 000 + 20 000 extra − 30 000 advance − 11 000 tax = 179 000; E2 150 000;
    // E3 80 000 + 10 000 commission − 20 000 advance = 70 000. Gross 460 000, net 399 000.
    let pr = f.ok(
        m::FIN_PAYROLL_CREATE,
        pay(json!([
            line(&e1, 200_000, 20_000, 0, 30_000, 11_000),
            line(&e2, 150_000, 0, 0, 0, 0),
            line(&e3, 80_000, 0, 10_000, 20_000, 0)
        ])),
    );
    assert!(pr["number"].as_str().unwrap().starts_with("PR-"));
    assert_eq!(
        (i(&pr["gross"]), i(&pr["advance"]), i(&pr["tax"]), i(&pr["net"])),
        (460_000, 50_000, 11_000, 399_000)
    );
    assert_eq!(
        f.rows(&pr["id"]),
        vec![
            (5110, 460_000),
            (2410, -11_000),
            (2310, -179_000),
            (1320, -30_000),
            (2310, -150_000),
            (2310, -70_000),
            (1320, -20_000)
        ]
    );
    // One payroll per period; the draft now shows the approved one.
    assert_eq!(
        f.fr(m::FIN_PAYROLL_CREATE, pay(json!([line(&e2, 1, 0, 0, 0, 0)]))),
        ("from".into(), "period_taken".into())
    );
    assert_eq!(f.ok(m::FIN_PAYROLL_DRAFT, json!({"from": today, "to": today}))["id"], pr["id"]);
    let e1i = f.employee_info(&e1);
    assert_eq!((i(&e1i["owed"]), i(&e1i["advances"])), (179_000, 20_000));
    assert_eq!(i(&f.employee_info(&e3)["advances"]), 0);

    // Salaries: never more than owed. E1 179 000 by bank, E2 150 000 and E3 70 000 from cash.
    assert_eq!(
        f.fr(
            m::FIN_VOUCHERS_CREATE,
            json!({"date": today, "account_id": 1110, "purpose": "salary", "employee_id": e2, "amount": 150_001})
        ),
        ("amount".into(), "payment_exceeds".into())
    );
    let s1 = f.ok(
        m::FIN_VOUCHERS_CREATE,
        json!({"date": today, "account_id": 1210, "purpose": "salary", "employee_id": e1, "amount": 179_000}),
    );
    assert_eq!(f.rows(&s1["id"]), vec![(2310, 179_000), (1210, -179_000)]);
    f.ok(m::FIN_VOUCHERS_CREATE, json!({"date": today, "account_id": 1110, "purpose": "salary", "employee_id": e2, "amount": 150_000}));
    f.ok(m::FIN_VOUCHERS_CREATE, json!({"date": today, "account_id": 1110, "purpose": "salary", "employee_id": e3, "amount": 70_000}));
    // Salary tax withheld goes to the government from the bank; it is never withheld on an expense.
    let tax = f.ok(
        m::FIN_VOUCHERS_CREATE,
        json!({"date": today, "account_id": 1210, "purpose": "tax", "withholding": "wage", "amount": 11_000}),
    );
    assert_eq!(f.rows(&tax["id"]), vec![(2410, 11_000), (1210, -11_000)]);
    assert_eq!(
        f.fr(
            m::FIN_VOUCHERS_CREATE,
            json!({"date": today, "account_id": 1110, "purpose": "expense",
                   "lines": [{"account_id": 5210, "amount": 1000, "withheld": 10, "withholding": "wage"}]})
        ),
        ("withholding".into(), "withheld_range".into())
    );

    // Paid documents cannot be voided away from under the payments.
    assert_eq!(
        f.fr(m::FIN_DOCS_VOID, json!({"id": pr["id"], "reason": "x"})),
        ("id".into(), "doc_not_voidable".into())
    );
    assert_eq!(
        f.fr(m::FIN_DOCS_VOID, json!({"id": adv3["id"], "reason": "x"})),
        ("id".into(), "doc_not_voidable".into())
    );

    // Cash 600 000 − 50 000 − 20 000 − 150 000 − 70 000 = 310 000; bank 300 000 − 179 000 − 11 000 = 110 000.
    assert_eq!((f.balance(1110), f.balance(1210)), (310_000, 110_000));
    let o = f.ok(m::FIN_OVERVIEW, json!({"from": today, "to": today}));
    assert_eq!(i(&o["cash"]["staff_paid"]), 469_000);
    assert_eq!(i(&o["cash"]["tax_paid"]), 11_000);
    assert_eq!(i(&o["cash"]["operating"]), -469_000);
    assert_eq!(i(&o["cash"]["money_closing"]) - i(&o["cash"]["money_opening"]), 420_000);
    assert_eq!(i(&o["accrual"]["expenses_total"]), 460_000);
    assert!(o["checks"].as_array().unwrap().iter().all(|c| c["ok"] == json!(true)), "{o}");

    // R8 and R9.
    let v = |c: &Value| i(&c["value"]);
    let r8 = f.report("payroll");
    assert_eq!(r8["rows"].as_array().unwrap().len(), 3);
    assert_eq!((v(&r8["totals"][9]), v(&r8["totals"][12])), (460_000, 399_000));
    let r9 = f.report("withholding");
    let row = &r9["rows"][0];
    assert_eq!(r9["rows"].as_array().unwrap().len(), 1);
    assert_eq!((row[1]["value"].as_str(), v(&row[2]), v(&row[3]), v(&row[4])), (Some("acct_2410"), 11_000, 11_000, 0));

    // An unpaid payroll of an earlier period can be voided; its period is free again.
    let old = json!({"from": day(-90), "to": day(-61), "date": today, "lines": [line(&e2, 150_000, 0, 0, 0, 0)]});
    let pr2 = f.ok(m::FIN_PAYROLL_CREATE, old.clone());
    assert_eq!(i(&f.employee_info(&e2)["owed"]), 150_000);
    f.ok(m::FIN_DOCS_VOID, json!({"id": pr2["id"], "reason": "wrong month"}));
    assert_eq!(i(&f.employee_info(&e2)["owed"]), 0);
    f.ok(m::FIN_PAYROLL_CREATE, old);
}

#[test]
fn a_share_statement_freezes_what_the_doctor_earned_and_is_paid_up_to_it() {
    let f = fx();
    let today = crate::clock::today_iso();
    let mut s = f.ok(m::FIN_SETTINGS_GET, json!({}));
    s["share_enabled"] = json!(true);
    s["share_basis"] = json!("treatment");
    f.ok(m::FIN_SETTINGS_SAVE, s);
    f.ok(m::FIN_SHARES_RULES_SAVE, json!({"doctor_id": f.doctor, "kind": "percent", "value": 2000}));
    // Invoice 100 000 − 10 000 discount = 90 000; the doctor's 20 % is 18 000.
    let t1 = f.done(&f.patient, 16, 100_000);
    let inv = f.ok(
        m::FIN_INVOICES_CREATE,
        json!({"patient_id": f.patient, "items": [{"item_id": t1}], "discount": 10_000}),
    );
    let pre = f.ok(m::FIN_SHARES_PREVIEW, json!({"from": today, "to": today}));
    assert_eq!((i(&pre["base_total"]), i(&pre["commission_total"])), (90_000, 18_000));

    let ds = f.ok(
        m::FIN_SHARES_STATEMENT_CREATE,
        json!({"doctor_id": f.doctor, "from": today, "to": today, "date": today}),
    );
    assert!(ds["number"].as_str().unwrap().starts_with("DS-"));
    assert_eq!((i(&ds["base"]), i(&ds["share"])), (90_000, 18_000));
    assert_eq!(ds["lines"].as_array().unwrap().len(), 1);
    assert_eq!(ds["lines"][0]["invoice_id"], inv["id"]);
    assert_eq!(f.rows(&ds["id"]), vec![(5120, 18_000), (2320, -18_000)]);
    assert_eq!(
        f.fr(m::FIN_SHARES_STATEMENT_CREATE, json!({"doctor_id": f.doctor, "from": today, "to": today, "date": today})),
        ("from".into(), "period_taken".into())
    );
    assert_eq!(
        f.fr(
            m::FIN_SHARES_STATEMENT_CREATE,
            json!({"doctor_id": f.doctor, "from": day(-200), "to": day(-199), "date": today})
        ),
        ("doctor_id".into(), "billing_amount".into())
    );

    // A later price reduction of 9 000 changes the preview (81 000 → 16 200), not the statement.
    f.ok(m::FIN_INVOICES_ADJUST, json!({"invoice_id": inv["id"], "kind": "reduction", "amount": 9_000, "reason": "x"}));
    let pre = f.ok(m::FIN_SHARES_PREVIEW, json!({"from": today, "to": today}));
    assert_eq!((i(&pre["base_total"]), i(&pre["commission_total"])), (81_000, 16_200));
    let again = f.ok(m::FIN_SHARES_STATEMENT_GET, json!({"id": ds["id"]}));
    assert_eq!((i(&again["base"]), i(&again["share"])), (90_000, 18_000));

    // Paid up to what is owed.
    let bal = f.ok(m::FIN_SHARES_BALANCES, json!({}));
    assert_eq!((i(&bal[0]["approved"]), i(&bal[0]["paid"]), i(&bal[0]["owed"])), (18_000, 0, 18_000));
    assert_eq!(
        f.fr(
            m::FIN_VOUCHERS_CREATE,
            json!({"date": today, "account_id": 1110, "purpose": "share", "doctor_id": f.doctor, "amount": 18_001})
        ),
        ("amount".into(), "payment_exceeds".into())
    );
    let paid = f.ok(
        m::FIN_VOUCHERS_CREATE,
        json!({"date": today, "account_id": 1110, "purpose": "share", "doctor_id": f.doctor, "amount": 18_000}),
    );
    assert_eq!(f.rows(&paid["id"]), vec![(2320, 18_000), (1110, -18_000)]);
    assert_eq!(paid["doctor_id"], json!(f.doctor));
    let bal = f.ok(m::FIN_SHARES_BALANCES, json!({}));
    assert_eq!((i(&bal[0]["approved"]), i(&bal[0]["paid"]), i(&bal[0]["owed"])), (18_000, 18_000, 0));
    assert_eq!(
        f.fr(m::FIN_DOCS_VOID, json!({"id": ds["id"], "reason": "x"})),
        ("id".into(), "doc_not_voidable".into())
    );
    assert_eq!(f.ok(m::FIN_SHARES_STATEMENTS, json!({"doctor_id": f.doctor})).as_array().unwrap().len(), 1);

    // Salaries and shares are for whoever holds the payroll permission: a receptionist does not,
    // an accountant does.
    f.ok(
        m::USERS_CREATE,
        json!({"username": "rec", "display_name": "پذیرش", "password": "rec-pass-123", "role": "receptionist"}),
    );
    f.ok(
        m::USERS_CREATE,
        json!({"username": "acc", "display_name": "حسابدار", "password": "acc-pass-123", "role": "accountant"}),
    );
    let rec = f.t.login("rec", "rec-pass-123");
    let acc = f.t.login("acc", "acc-pass-123");
    assert_eq!(
        f.t.call(m::FIN_EMPLOYEES_LIST, json!({}), Some(&rec)).unwrap_err(),
        ErrorCode::Forbidden
    );
    assert_eq!(
        f.t.call(
            m::FIN_VOUCHERS_CREATE,
            json!({"date": today, "account_id": 1110, "purpose": "share", "doctor_id": f.doctor, "amount": 1}),
            Some(&rec)
        )
        .unwrap_err(),
        ErrorCode::Forbidden
    );
    assert!(f.t.call(m::FIN_PAYROLL_LIST, json!({}), Some(&acc)).is_ok());
}
