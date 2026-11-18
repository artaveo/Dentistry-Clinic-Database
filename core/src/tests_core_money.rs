//! Tests for the fixes of the money / plans / chart test mission (CT-*, FR-* ids in the comments;
//! see docs/test-mission/FIXES-core-money.md).

use artaveo_shared::{methods as m, ErrorCode, RpcRequest, RpcResponse};
use serde_json::{json, Value};

use crate::tests::T;

fn failure(t: &T, method: &str, params: Value, token: &str) -> (ErrorCode, Option<String>, Option<String>) {
    match t.core.handle(RpcRequest { method: method.into(), params, token: Some(token.into()) }) {
        RpcResponse::Ok { result } => panic!("{method} unexpectedly succeeded: {result}"),
        RpcResponse::Error { error } => (
            error.code,
            error.field.clone(),
            error.rule.map(|r| serde_json::to_value(r).unwrap().as_str().unwrap().to_string()),
        ),
    }
}

/// (field, rule) of a call that must fail.
fn fr(t: &T, method: &str, params: Value, token: &str) -> (String, String) {
    let (_, f, r) = failure(t, method, params, token);
    (f.unwrap_or_default(), r.unwrap_or_default())
}

fn day(n: i64) -> String {
    let d = crate::scheduling::parse_date("d", &crate::clock::today_iso()).unwrap() + time::Duration::days(n);
    crate::scheduling::format_date(d)
}

struct Fx {
    t: T,
    owner: String,
    patient: String,
}

fn fx() -> Fx {
    let t = T::new();
    t.setup();
    let owner = t.login("owner", "owner-pass-1");
    let p = t.ok(m::PATIENTS_CREATE, json!({"full_name": "Money Test"}), Some(&owner));
    let patient = p["id"].as_str().unwrap().to_string();
    Fx { t, owner, patient }
}

impl Fx {
    fn service(&self, code: &str) -> String {
        self.t.ok(m::CATALOG_GET, json!({}), Some(&self.owner))["services"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["code"] == code)
            .unwrap_or_else(|| panic!("service {code}"))["id"]
            .as_str()
            .unwrap()
            .to_string()
    }
    fn condition(&self, code: &str) -> String {
        self.t
            .ok(m::TOOTH_CONDITIONS_LIST, json!({}), Some(&self.owner))
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["code"] == code)
            .unwrap_or_else(|| panic!("condition {code}"))["id"]
            .as_str()
            .unwrap()
            .to_string()
    }
    fn plan(&self, title: &str) -> Value {
        self.t.ok(m::PLANS_CREATE, json!({"patient_id": self.patient, "title": title}), Some(&self.owner))
    }
    fn add(&self, plan: Option<&str>, code: &str, teeth: Value, extra: Value) -> Value {
        let mut p = json!({"patient_id": self.patient, "plan_id": plan, "service_id": self.service(code),
                           "teeth": teeth, "unit_price": 100_000});
        for (k, v) in extra.as_object().unwrap() {
            p[k] = v.clone();
        }
        self.t.ok(m::TREATMENT_ITEMS_ADD, p, Some(&self.owner))[0].clone()
    }
    fn try_add(&self, plan: Option<&str>, code: &str, teeth: Value, extra: Value) -> (String, String) {
        let mut p = json!({"patient_id": self.patient, "plan_id": plan, "service_id": self.service(code),
                           "teeth": teeth, "unit_price": 100_000});
        for (k, v) in extra.as_object().unwrap() {
            p[k] = v.clone();
        }
        fr(&self.t, m::TREATMENT_ITEMS_ADD, p, &self.owner)
    }
    fn session(&self, item: &Value) -> Value {
        self.t.ok(
            m::TREATMENT_ITEMS_RECORD_SESSION,
            json!({"item_id": item["id"], "version": item["version"]}),
            Some(&self.owner),
        )
    }
    fn item_status(&self, item: &Value, status: &str, reason: Option<&str>) -> Value {
        self.t.ok(
            m::TREATMENT_ITEMS_SET_STATUS,
            json!({"id": item["id"], "version": item["version"], "status": status, "reason": reason}),
            Some(&self.owner),
        )
    }
    fn plans(&self) -> Vec<Value> {
        self.t
            .ok(m::PLANS_LIST, json!({"patient_id": self.patient}), Some(&self.owner))
            .as_array()
            .unwrap()
            .clone()
    }
}

// ───────────────────────────── plans and items ─────────────────────────────

#[test]
fn ct05_a_multi_session_item_cannot_be_completed_without_a_session() {
    let f = fx();
    let plan = f.plan("p");
    let item = f.add(plan["id"].as_str(), "END-01", json!([36]), json!({"sessions": 2}));
    assert_eq!(
        fr(
            &f.t,
            m::TREATMENT_ITEMS_SET_STATUS,
            json!({"id": item["id"], "version": item["version"], "status": "completed"}),
            &f.owner
        ),
        ("status".into(), "item_sessions".into())
    );
    assert!(f
        .t
        .ok(m::FIN_INVOICES_BILLABLE, json!({"patient_id": f.patient}), Some(&f.owner))
        .as_array()
        .unwrap()
        .is_empty());
    // After one session it may be finished early (the doctor's call).
    let done = f.session(&item);
    let item = done["item"].clone();
    assert_eq!(f.item_status(&item, "completed", None)["status"], "completed");
    // A one-session item can still be completed directly.
    let single = f.add(plan["id"].as_str(), "RST-01C", json!([16]), json!({"surfaces": "O"}));
    assert_eq!(f.item_status(&single, "completed", None)["status"], "completed");
}

#[test]
fn ct06_cancelling_a_plan_needs_a_reason_and_ct28_reopening_restores_its_items() {
    let f = fx();
    let plan = f.plan("p");
    f.add(plan["id"].as_str(), "RST-01C", json!([16]), json!({"surfaces": "O"}));
    f.add(plan["id"].as_str(), "SRG-01", json!([48]), json!({}));
    let set = |status: &str, version: &Value, reason: Option<&str>| {
        f.t.ok(
            m::PLANS_SET_STATUS,
            json!({"id": plan["id"], "version": version, "status": status, "reason": reason}),
            Some(&f.owner),
        )
    };
    let p = f.plans()[0].clone();
    assert_eq!(
        fr(
            &f.t,
            m::PLANS_SET_STATUS,
            json!({"id": plan["id"], "version": p["version"], "status": "cancelled"}),
            &f.owner
        ),
        ("reason".into(), "required".into())
    );
    let cancelled = set("cancelled", &p["version"], Some("patient left"));
    assert_eq!(cancelled["status"], "cancelled");
    assert!(cancelled["items"].as_array().unwrap().iter().all(|i| i["status"] == "cancelled"));
    let reopened = set("proposed", &cancelled["version"], None);
    assert_eq!(reopened["status"], "proposed");
    assert_eq!(reopened["items"].as_array().unwrap().len(), 2);
    assert!(reopened["items"].as_array().unwrap().iter().all(|i| i["status"] == "planned"));
    assert_eq!(reopened["total"], 200_000, "the plan has its price back");
}

#[test]
fn ct07_a_plan_that_closed_itself_takes_new_items_and_reopens() {
    let f = fx();
    let plan = f.plan("p");
    f.t.ok(
        m::TREATMENT_ITEMS_ADD,
        json!({"patient_id": f.patient, "plan_id": plan["id"], "service_id": f.service("SRG-01"),
               "teeth": [48], "unit_price": 100_000, "perform_now": true}),
        Some(&f.owner),
    );
    assert_eq!(f.plans()[0]["status"], "completed", "its only item is done");
    f.add(plan["id"].as_str(), "SRG-01", json!([38]), json!({}));
    let p = f.plans()[0].clone();
    assert_eq!(p["status"], "in_progress");
    assert_eq!(p["items"].as_array().unwrap().len(), 2);
    // A cancelled plan stays closed.
    let c = f.t.ok(
        m::PLANS_SET_STATUS,
        json!({"id": p["id"], "version": p["version"], "status": "cancelled", "reason": "x"}),
        Some(&f.owner),
    );
    assert_eq!(c["status"], "cancelled");
    assert_eq!(
        f.try_add(plan["id"].as_str(), "SRG-01", json!([28]), json!({})),
        ("plan_id".into(), "plan_not_editable".into())
    );
}

#[test]
fn ct10_ct11_ct12_ct13_items_are_checked_against_dates_teeth_and_the_chart() {
    let f = fx();
    let plan = f.plan("p");
    let pid = plan["id"].as_str();
    // CT-10
    assert_eq!(
        f.try_add(pid, "SRG-01", json!([48]), json!({"est_start": day(10), "est_end": day(5)})),
        ("est_end".into(), "period_range".into())
    );
    // CT-11: a bridge is two or more teeth in one jaw.
    assert_eq!(f.try_add(pid, "PRS-02Z", json!([16]), json!({})), ("teeth".into(), "item_teeth".into()));
    assert_eq!(f.try_add(pid, "PRS-02Z", json!([16, 36]), json!({})), ("teeth".into(), "item_teeth".into()));
    f.add(pid, "PRS-02Z", json!([15, 16, 17]), json!({}));
    // CT-13: the same service on the same tooth twice in one plan.
    f.add(pid, "SRG-01", json!([48]), json!({}));
    assert_eq!(
        f.try_add(pid, "SRG-01", json!([48]), json!({})),
        ("service_id".into(), "item_duplicate".into())
    );
    f.add(pid, "SRG-01", json!([38]), json!({}));
    // CT-12: no extraction on a tooth the chart records as missing.
    let missing = f.condition("missing");
    f.t.ok(
        m::CHART_ADD_FINDINGS,
        json!({"patient_id": f.patient, "condition_id": missing, "teeth": [28]}),
        Some(&f.owner),
    );
    assert_eq!(f.try_add(pid, "SRG-01", json!([28]), json!({})), ("teeth".into(), "tooth_conflict".into()));
    // CT-04: a baby tooth is not planned on an adult chart.
    assert_eq!(f.try_add(pid, "SRG-01", json!([55]), json!({})), ("teeth".into(), "tooth_number".into()));
}

// ───────────────────────────── chart ─────────────────────────────

#[test]
fn ct02_ct03_ct04_ct16_chart_findings_are_checked() {
    let f = fx();
    let caries = f.condition("caries");
    let add = |condition: &str, teeth: Value, surfaces: &str| {
        f.t.ok(
            m::CHART_ADD_FINDINGS,
            json!({"patient_id": f.patient, "condition_id": condition, "teeth": teeth, "surfaces": surfaces}),
            Some(&f.owner),
        )
    };
    // CT-02: the same finding twice is one finding.
    let a = add(&caries, json!([16]), "MO");
    let b = add(&caries, json!([16]), "MO");
    assert_eq!(a[0]["id"], b[0]["id"]);
    let chart = f.t.ok(m::CHART_GET, json!({"patient_id": f.patient}), Some(&f.owner));
    assert_eq!(chart["findings"].as_array().unwrap().len(), 1);
    add(&caries, json!([16]), "D");
    assert_eq!(
        f.t.ok(m::CHART_GET, json!({"patient_id": f.patient}), Some(&f.owner))["findings"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    // CT-03: nothing is found on a missing tooth; an implant on it is fine.
    let missing = f.condition("missing");
    add(&missing, json!([46]), "");
    let err = fr(
        &f.t,
        m::CHART_ADD_FINDINGS,
        json!({"patient_id": f.patient, "condition_id": caries, "teeth": [46], "surfaces": "O"}),
        &f.owner,
    );
    assert_eq!(err, ("teeth".into(), "tooth_conflict".into()));
    add(&f.condition("implant"), json!([46]), "");
    add(&caries, json!([46]), "O");

    // CT-04: no baby tooth on an adult's chart.
    let err = fr(
        &f.t,
        m::CHART_ADD_FINDINGS,
        json!({"patient_id": f.patient, "condition_id": caries, "teeth": [55], "surfaces": "O"}),
        &f.owner,
    );
    assert_eq!(err, ("teeth".into(), "tooth_number".into()));

    // CT-16: removing a finding twice says "finding not found".
    let id = a[0]["id"].clone();
    let version = a[0]["version"].clone();
    f.t.ok(m::CHART_REMOVE_FINDING, json!({"id": id, "version": version}), Some(&f.owner));
    let err = fr(&f.t, m::CHART_REMOVE_FINDING, json!({"id": id, "version": version}), &f.owner);
    assert_eq!(err, ("id".into(), "finding_not_found".into()));
}

// ───────────────────────────── billing ─────────────────────────────

#[test]
fn ct08_a_started_orthodontic_course_can_be_invoiced_with_installments() {
    let f = fx();
    let plan = f.plan("braces");
    let item =
        f.add(plan["id"].as_str(), "ORT-01C", json!([]), json!({"sessions": 24, "unit_price": 6_000_000}));
    // No session yet: not billable.
    assert!(f
        .t
        .ok(m::FIN_INVOICES_BILLABLE, json!({"patient_id": f.patient}), Some(&f.owner))
        .as_array()
        .unwrap()
        .is_empty());
    let (_, field, rule) = failure(
        &f.t,
        m::FIN_INVOICES_CREATE,
        json!({"patient_id": f.patient, "items": [{"item_id": item["id"]}]}),
        &f.owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("items"), Some("item_not_billable")));
    // After the first session it is, as a whole, with its installments.
    f.session(&item);
    let billable = f.t.ok(m::FIN_INVOICES_BILLABLE, json!({"patient_id": f.patient}), Some(&f.owner));
    assert_eq!(billable.as_array().unwrap().len(), 1);
    assert_eq!(billable[0]["completed"], false);
    assert_eq!(
        (billable[0]["sessions_done"].as_i64(), billable[0]["sessions_planned"].as_i64()),
        (Some(1), Some(24))
    );
    let inv = f.t.ok(
        m::FIN_INVOICES_CREATE,
        json!({"patient_id": f.patient, "items": [{"item_id": item["id"]}],
               "installments": [{"due_date": day(30), "amount": 3_000_000}, {"due_date": day(60), "amount": 3_000_000}]}),
        Some(&f.owner),
    );
    assert_eq!(inv["net"], 6_000_000);
    // Not twice.
    assert!(f
        .t
        .ok(m::FIN_INVOICES_BILLABLE, json!({"patient_id": f.patient}), Some(&f.owner))
        .as_array()
        .unwrap()
        .is_empty());
    let (_, _, rule) = failure(
        &f.t,
        m::FIN_INVOICES_CREATE,
        json!({"patient_id": f.patient, "items": [{"item_id": item["id"]}]}),
        &f.owner,
    );
    assert_eq!(rule.as_deref(), Some("item_already_billed"));
    // An invoiced course keeps its price and cannot be cancelled (void the invoice first).
    let item = f.t.ok(m::PLANS_LIST, json!({"patient_id": f.patient}), Some(&f.owner))[0]["items"][0].clone();
    assert_eq!(
        fr(
            &f.t,
            m::TREATMENT_ITEMS_SET_STATUS,
            json!({"id": item["id"], "version": item["version"], "status": "cancelled", "reason": "x"}),
            &f.owner
        ),
        ("id".into(), "item_not_editable".into())
    );
}

#[test]
fn ct09_fr04_installments_follow_credit_notes_and_reject_typo_dates() {
    let f = fx();
    let inv = f.t.ok(
        m::FIN_INVOICES_CREATE,
        json!({"patient_id": f.patient,
               "extra_lines": [{"description": "work", "quantity": 1, "unit_price": 1_000_000}],
               "installments": [{"due_date": day(10), "amount": 333_333}, {"due_date": day(40), "amount": 333_333},
                                {"due_date": day(70), "amount": 333_334}]}),
        Some(&f.owner),
    );
    // CT-09: a due date years back is a typo; yesterday is a late installment and still allowed.
    let (_, field, rule) = failure(
        &f.t,
        m::FIN_INVOICES_SET_PLAN,
        json!({"invoice_id": inv["id"], "reason": "new dates",
               "installments": [{"due_date": "2020-01-01", "amount": 1_000_000}]}),
        &f.owner,
    );
    assert_eq!((field.as_deref(), rule.as_deref()), (Some("installments"), Some("installments_invalid")));
    // FR-04: a price reduction of 500 000 takes the last two installments' worth off the plan.
    let after = f.t.ok(
        m::FIN_INVOICES_ADJUST,
        json!({"invoice_id": inv["id"], "kind": "reduction", "amount": 500_000, "reason": "goodwill"}),
        Some(&f.owner),
    );
    assert_eq!(after["remaining"], 500_000);
    let plan = after["plan"]["installments"].as_array().unwrap();
    let sum: i64 = plan.iter().map(|x| x["amount"].as_i64().unwrap()).sum();
    assert_eq!(sum, 500_000, "the plan adds up to what is still owed");
    assert_eq!(plan.len(), 2, "the last installment went away entirely");
    assert_eq!(plan[0]["amount"], 333_333);
    assert_eq!(plan[1]["amount"], 166_667);
}

// ───────────────────────────── reports ─────────────────────────────

#[test]
fn fr05_fr08_fr09_reports_use_net_amounts_and_exports_never_overwrite() {
    let f = fx();
    let doc = f.t.ok(m::DOCTORS_CREATE, json!({"full_name": "Dr A", "color": "#0E7490"}), Some(&f.owner));
    let inv = f.t.ok(
        m::FIN_INVOICES_CREATE,
        json!({"patient_id": f.patient, "discount": 20_000,
               "extra_lines": [{"description": "a", "quantity": 1, "unit_price": 60_000, "doctor_id": doc["id"]},
                               {"description": "b", "quantity": 1, "unit_price": 40_000, "doctor_id": doc["id"]}]}),
        Some(&f.owner),
    );
    f.t.ok(
        m::FIN_INVOICES_ADJUST,
        json!({"invoice_id": inv["id"], "kind": "reduction", "amount": 10_000, "reason": "r"}),
        Some(&f.owner),
    );
    let (from, to) = (day(-1), day(1));
    let report =
        |kind: &str| f.t.ok(m::REPORTS_RUN, json!({"kind": kind, "from": from, "to": to}), Some(&f.owner));
    // FR-05: net = 100 000 - 20 000 discount - 10 000 price reduction.
    let rows = report("provider_production");
    assert_eq!(rows["rows"][0][5]["value"], 70_000);
    // FR-08: the adjustments report lists the price reduction too.
    let d = report("adjustments");
    let amounts: Vec<i64> =
        d["rows"].as_array().unwrap().iter().map(|r| r[4]["value"].as_i64().unwrap()).collect();
    assert_eq!(amounts.iter().sum::<i64>(), 30_000);
    // FR-09: two exports in the same second are two files.
    let export = || {
        f.t.ok(
            m::REPORTS_EXPORT,
            json!({"report": {"kind": "adjustments", "from": from, "to": to}, "language": "en", "title": "t",
                   "headers": ["a", "b", "c", "d", "e", "f", "g"]}),
            Some(&f.owner),
        )
    };
    let (one, two) = (export(), export());
    assert_ne!(one["file_name"], two["file_name"]);
    assert!(std::path::Path::new(one["file_path"].as_str().unwrap()).exists());
    assert!(std::path::Path::new(two["file_path"].as_str().unwrap()).exists());
}

// ───────────────────────────── documents ─────────────────────────────

#[test]
fn ct23_ct26_prescriptions_have_no_duplicate_medicine_and_voided_ones_cannot_be_printed() {
    let f = fx();
    let doc = f.t.ok(m::DOCTORS_CREATE, json!({"full_name": "Dr A", "color": "#0E7490"}), Some(&f.owner));
    let line = |name: &str| json!({"name": name, "form": "tablet", "strength": "500 mg", "dose": "1", "times_per_day": 3, "days": 5});
    let issue = |items: Vec<Value>| {
        f.t.core.handle(RpcRequest {
            method: m::DOCUMENTS_ISSUE.into(),
            params: json!({"patient_id": f.patient, "doctor_id": doc["id"], "language": "en", "paper": "a5",
                           "content": {"kind": "prescription", "items": items, "acknowledged": []}}),
            token: Some(f.owner.clone()),
        })
    };
    match issue(vec![line("Amoxicillin"), line("amoxicillin")]) {
        RpcResponse::Error { error } => assert_eq!(error.field.as_deref(), Some("items.1.name")),
        RpcResponse::Ok { .. } => panic!("duplicate medicine accepted"),
    }
    let RpcResponse::Ok { result } = issue(vec![line("Amoxicillin"), line("Ibuprofen")]) else {
        panic!("issue failed")
    };
    let void = f.t.ok(
        m::DOCUMENTS_VOID,
        json!({"id": result["id"], "version": result["version"], "reason": "mistake"}),
        Some(&f.owner),
    );
    assert_eq!(void["status"], "void");
    let (_, rule) = fr(&f.t, m::DOCUMENTS_MARK_PRINTED, json!({"id": result["id"], "pdf": false}), &f.owner);
    assert_eq!(rule, "document_void");
}
