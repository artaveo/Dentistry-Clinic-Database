// Artaveo Vendor Panel (roadmap 10.11). Static page: signs in with Supabase Auth; what a vendor may
// read or change is decided by the Row Level Security policies, not by this file.
import { createClient } from "https://esm.sh/@supabase/supabase-js@2.45.4";
import { PLANS, generateActivationCode, renewedExpiry, sha256Hex } from "../supabase/functions/_shared/license.mjs";

const cfg = window.ARTAVEO_PANEL;
const sb = createClient(cfg.supabaseUrl, cfg.publishableKey);
const $ = (id) => document.getElementById(id);

/** Tiny DOM builder: text always goes through textContent, never innerHTML. */
function h(tag, props = {}, ...kids) {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (k === "class") node.className = v;
    else if (k.startsWith("on")) node.addEventListener(k.slice(2), v);
    else if (v !== false && v != null) node.setAttribute(k, v === true ? "" : v);
  }
  for (const kid of kids.flat()) node.append(kid instanceof Node ? kid : document.createTextNode(String(kid ?? "")));
  return node;
}

const fmt = (iso) => (iso ? new Date(iso).toISOString().slice(0, 16).replace("T", " ") : "—");
const day = (iso) => (iso ? iso.slice(0, 10) : "—");
const say = (msg, bad = false) => {
  const n = $("notice");
  n.hidden = !msg;
  n.textContent = msg;
  n.style.color = bad ? "var(--danger)" : "var(--ink)";
};
const run = async (fn) => {
  say("");
  try {
    return await fn();
  } catch (e) {
    say(e?.message ?? String(e), true);
  }
};
const must = ({ data, error }) => {
  if (error) throw new Error(error.message);
  return data;
};

function table(columns, rows, actions) {
  return h("div", { class: "table-wrap" }, h("table", {},
    h("thead", {}, h("tr", {}, columns.map(([label]) => h("th", {}, label)), actions ? h("th") : [])),
    h("tbody", {}, rows.map((row) => h("tr", {},
      columns.map(([, get]) => h("td", {}, get(row))),
      actions ? h("td", {}, actions(row)) : [])))));
}

const tag = (text, kind = "") => h("span", { class: `tag ${kind}` }, text);
const field = (label, input) => h("label", {}, label, input);
const select = (name, options) => h("select", { name }, options.map(([v, l]) => h("option", { value: v }, l)));
const formData = (form) => Object.fromEntries(new FormData(form).entries());

// ───────────────────────────── sections ─────────────────────────────

async function clinicsView(view) {
  const clinics = must(await sb.from("clinics").select("*").order("created_at", { ascending: false }));
  const form = h("form", { class: "form-row", onsubmit: (e) => {
    e.preventDefault();
    const f = formData(e.target);
    run(async () => {
      must(await sb.from("clinics").insert({ name: f.name, owner_name: f.owner_name || null, phone: f.phone || null, city: f.city || null }));
      show("clinics");
    });
  } },
  field("Clinic name", h("input", { name: "name", required: true })),
  field("Owner", h("input", { name: "owner_name" })),
  field("Phone", h("input", { name: "phone" })),
  field("City", h("input", { name: "city" })),
  h("button", { class: "primary", type: "submit" }, "Add clinic"));
  view.append(
    h("section", { class: "card" }, h("h2", {}, "New clinic"), form),
    h("section", { class: "card" }, h("h2", {}, `Clinics (${clinics.length})`),
      table([["Name", (c) => c.name], ["Owner", (c) => c.owner_name ?? "—"], ["Phone", (c) => c.phone ?? "—"], ["City", (c) => c.city ?? "—"], ["Added", (c) => day(c.created_at)]], clinics)),
  );
}

async function licensesView(view) {
  const clinics = must(await sb.from("clinics").select("id,name").order("name"));
  const licenses = must(await sb.from("licenses").select("*, clinics(name)").order("created_at", { ascending: false }));
  const planPrice = { basic: "3,000", standard: "5,000", pro: "7,500" };
  const form = h("form", { class: "form-row", onsubmit: (e) => {
    e.preventDefault();
    const f = formData(e.target);
    run(async () => {
      const starts = new Date().toISOString();
      must(await sb.from("licenses").insert({ clinic_id: f.clinic, plan: f.plan, starts_at: starts, expires_at: renewedExpiry(null, starts, Number(f.months)) }));
      show("licenses");
    });
  } },
  field("Clinic", select("clinic", clinics.map((c) => [c.id, c.name]))),
  field("Plan", select("plan", Object.keys(PLANS).filter((p) => p !== "trial").map((p) => [p, `${p} — ${PLANS[p].computers} PC — ${planPrice[p]} AFN/yr`]))),
  field("Months", h("input", { name: "months", type: "number", min: 1, max: 60, value: 12 })),
  h("button", { class: "primary", type: "submit" }, "Issue license"));

  const renew = (l) => run(async () => {
    const months = Number(prompt("Renew for how many months?", "12"));
    if (!months) return;
    must(await sb.from("licenses").update({ expires_at: renewedExpiry(l.expires_at, new Date().toISOString(), months), updated_at: new Date().toISOString() }).eq("id", l.id));
    show("licenses");
  });
  const revoke = (l) => run(async () => {
    const reason = prompt("Reason for revoking (kept for support):");
    if (!reason) return;
    must(await sb.from("licenses").update({ status: "revoked", revoked_reason: reason, updated_at: new Date().toISOString() }).eq("id", l.id));
    show("licenses");
  });
  const restore = (l) => run(async () => {
    must(await sb.from("licenses").update({ status: "active", revoked_reason: null, updated_at: new Date().toISOString() }).eq("id", l.id));
    show("licenses");
  });
  const code = (l) => run(async () => {
    const plain = generateActivationCode();
    const days = Number(prompt("Code valid for how many days?", "14"));
    if (!days) return;
    must(await sb.from("activation_codes").insert({
      code_hash: await sha256Hex(plain), code_hint: plain.slice(0, 9), clinic_id: l.clinic_id, license_id: l.id,
      max_uses: PLANS[l.plan].computers, expires_at: new Date(Date.now() + days * 86_400_000).toISOString(),
    }));
    view.prepend(h("section", { class: "card" }, h("h2", {}, "Activation code (shown once — send it to the clinic)"), h("div", { class: "big-code mono" }, plain)));
  });

  view.append(
    h("section", { class: "card" }, h("h2", {}, "New license"), form),
    h("section", { class: "card" }, h("h2", {}, `Licenses (${licenses.length})`),
      table([
        ["Clinic", (l) => l.clinics?.name ?? "—"], ["Plan", (l) => `${l.plan} (${PLANS[l.plan].computers} PC)`],
        ["Status", (l) => tag(l.status, l.status === "active" ? "ok" : "bad")], ["Starts", (l) => day(l.starts_at)],
        ["Expires", (l) => tag(day(l.expires_at), new Date(l.expires_at) < new Date() ? "bad" : "")],
      ], licenses, (l) => h("span", { class: "form-row" },
        h("button", { onclick: () => renew(l) }, "Renew"),
        h("button", { onclick: () => code(l) }, "Activation code"),
        l.status === "active" ? h("button", { class: "danger", onclick: () => revoke(l) }, "Revoke") : h("button", { onclick: () => restore(l) }, "Reinstate")))),
  );
}

async function codesView(view) {
  const codes = must(await sb.from("activation_codes").select("*, clinics(name)").order("created_at", { ascending: false }).limit(200));
  view.append(h("section", { class: "card" }, h("h2", {}, "Activation codes"),
    h("p", { class: "muted" }, "Codes are stored hashed: only the first characters are kept to recognise one. Create a code from the Licenses tab."),
    table([
      ["Clinic", (c) => c.clinics?.name ?? "—"], ["Code", (c) => h("code", {}, `${c.code_hint}…`)],
      ["Used", (c) => `${c.used}/${c.max_uses}`], ["Valid until", (c) => day(c.expires_at)],
      ["State", (c) => c.revoked_at ? tag("revoked", "bad") : new Date(c.expires_at) < new Date() ? tag("expired", "warn") : tag("valid", "ok")],
    ], codes, (c) => !c.revoked_at && h("button", { class: "danger", onclick: () => run(async () => {
      must(await sb.from("activation_codes").update({ revoked_at: new Date().toISOString() }).eq("id", c.id));
      show("codes");
    }) }, "Revoke"))));
}

async function installationsView(view) {
  const rows = must(await sb.from("installations").select("*, clinics(name)").order("last_seen", { ascending: false }).limit(300));
  const setStatus = (i, status) => run(async () => {
    must(await sb.from("installations").update({ status }).eq("id", i.id));
    show("installations");
  });
  view.append(h("section", { class: "card" }, h("h2", {}, `Installations (${rows.length})`),
    table([
      ["Clinic", (i) => i.clinics?.name ?? i.clinic_name ?? "—"], ["Computer", (i) => i.computer_name ?? "—"],
      ["Version", (i) => i.app_version ?? "—"], ["Status", (i) => tag(i.status, i.status === "active" ? "ok" : i.status === "blocked" ? "bad" : "")],
      ["Last seen", (i) => fmt(i.last_seen)],
      ["Last backup", (i) => i.last_backup_at ? tag(day(i.last_backup_at), i.last_backup_ok ? "ok" : "bad") : "—"],
      ["Trial ends", (i) => day(i.trial_expires_at)],
    ], rows, (i) => i.status === "blocked"
      ? h("button", { onclick: () => setStatus(i, "active") }, "Unblock")
      : h("button", { class: "danger", onclick: () => setStatus(i, "blocked") }, "Block"))));
}

async function eventsView(view) {
  const rows = must(await sb.from("security_events").select("*").order("at", { ascending: false }).limit(300));
  view.append(h("section", { class: "card" }, h("h2", {}, "Security events"),
    table([["When", (e) => fmt(e.at)], ["Event", (e) => e.kind], ["Installation", (e) => h("code", {}, (e.installation_key ?? "").slice(0, 13))], ["Detail", (e) => JSON.stringify(e.detail)]], rows)));
}

async function offlineView(view) {
  const licenses = must(await sb.from("licenses").select("id, plan, clinics(name)").eq("status", "active"));
  const out = h("div");
  const form = h("form", { onsubmit: (e) => {
    e.preventDefault();
    const f = formData(e.target);
    run(async () => {
      const { data, error } = await sb.functions.invoke("license-offline", { body: { request_code: f.request_code.trim(), license_id: f.license } });
      if (error) throw new Error(error.message);
      if (data?.error) throw new Error(data.error);
      out.replaceChildren(h("h2", {}, "Response code — send it to the clinic"), h("textarea", { readonly: true, rows: 6, class: "mono", style: "width:100%" }, data.response_code));
    });
  } },
  field("Clinic license", select("license", licenses.map((l) => [l.id, `${l.clinics?.name ?? "?"} — ${l.plan}`]))),
  field("Request code from the clinic's computer", h("textarea", { name: "request_code", rows: 4, required: true, class: "mono" })),
  h("button", { class: "primary", type: "submit" }, "Create response code"), out);
  view.append(h("section", { class: "card" }, h("h2", {}, "Offline activation"), form));
}

async function releasesView(view) {
  const rows = must(await sb.from("app_releases").select("*").order("released_at", { ascending: false }));
  const form = h("form", { class: "form-row", onsubmit: (e) => {
    e.preventDefault();
    const f = formData(e.target);
    run(async () => {
      must(await sb.from("app_releases").upsert({ version: f.version, min_supported: f.min_supported || null, download_url: f.download_url || null, notes: f.notes || null, channel: f.channel }));
      show("releases");
    });
  } },
  field("Version", h("input", { name: "version", required: true, pattern: "\\d+\\.\\d+\\.\\d+", placeholder: "0.11.0" })),
  field("Oldest allowed", h("input", { name: "min_supported", placeholder: "0.10.0" })),
  field("Download page", h("input", { name: "download_url", type: "url" })),
  field("Channel", select("channel", [["stable", "stable"], ["beta", "beta"]])),
  field("Notes", h("input", { name: "notes" })),
  h("button", { class: "primary", type: "submit" }, "Publish"));
  view.append(h("section", { class: "card" }, h("h2", {}, "Publish a version"), form),
    h("section", { class: "card" }, h("h2", {}, "Versions"),
      table([["Version", (r) => r.version], ["Channel", (r) => r.channel], ["Oldest allowed", (r) => r.min_supported ?? "—"], ["Released", (r) => day(r.released_at)], ["Notes", (r) => r.notes ?? ""]], rows)));
}

async function exportView(view) {
  const button = h("button", { class: "primary", onclick: () => run(async () => {
    const out = {};
    for (const t of ["clinics", "licenses", "installations", "activation_codes", "security_events", "app_releases"]) out[t] = must(await sb.from(t).select("*"));
    out.exported_at = new Date().toISOString();
    const url = URL.createObjectURL(new Blob([JSON.stringify(out, null, 2)], { type: "application/json" }));
    const a = h("a", { href: url, download: `artaveo-license-db-${day(out.exported_at)}.json` });
    document.body.append(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
    say("Exported. Keep the file somewhere safe: the free Supabase plan has no automatic backup.");
  }) }, "Download export");
  view.append(h("section", { class: "card" }, h("h2", {}, "Export the license database"),
    h("p", { class: "muted" }, "Do this regularly (for example every week). The free Supabase plan does not back up automatically. The file holds clinic names and license data, so store it like a password."),
    button));
}

const SECTIONS = {
  clinics: ["Clinics", clinicsView], licenses: ["Licenses", licensesView], codes: ["Codes", codesView],
  installations: ["Installations", installationsView], events: ["Security", eventsView],
  offline: ["Offline activation", offlineView], releases: ["Releases", releasesView], export: ["Export", exportView],
};

let current = "licenses";
async function show(name) {
  current = name;
  $("tabs").replaceChildren(...Object.entries(SECTIONS).map(([key, [label]]) =>
    h("button", { "aria-current": key === name ? "page" : null, onclick: () => show(key) }, label)));
  const view = $("view");
  view.replaceChildren();
  await run(() => SECTIONS[name][1](view));
}

// ───────────────────────────── sign-in ─────────────────────────────

async function enter() {
  const { data } = await sb.auth.getSession();
  const user = data.session?.user;
  if (!user) {
    $("login").hidden = false;
    $("app").hidden = true;
    return;
  }
  const { data: vendor } = await sb.from("vendor_users").select("display_name").eq("user_id", user.id).maybeSingle();
  if (!vendor) {
    await sb.auth.signOut();
    $("login-error").textContent = "This account is not a vendor account.";
    $("login").hidden = false;
    $("app").hidden = true;
    return;
  }
  $("login").hidden = true;
  $("app").hidden = false;
  $("who").textContent = vendor.display_name;
  show(current);
}

$("login-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const f = formData(e.target);
  const { error } = await sb.auth.signInWithPassword({ email: f.email, password: f.password });
  $("login-error").textContent = error ? "Wrong email or password." : "";
  if (!error) enter();
});
$("logout").addEventListener("click", async () => {
  await sb.auth.signOut();
  enter();
});
enter();
