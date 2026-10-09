//! Recipient pages, notices and email text. Preview and send both come through
//! here, so what an operator previews is what a client receives. Every value
//! interpolated into HTML is escaped with the invoicing `esc`; the inline script is
//! fixed text that reads its values from `data-*` attributes.
use super::model::RecipientRole;
use crate::invoicing::render_html::esc;

pub struct PageContext<'a> {
    pub company: &'a str,
    pub client_name: &'a str,
    pub kind: &'a str,
    pub title: &'a str,
    pub token: &'a str,
    pub version: i64,
    pub checksum: &'a str,
    pub pdf_href: &'a str,
    /// The letterhead logo's `<img src>`: the address invoices published it
    /// at, or the image inline as a `data:` URI. `None` draws no image.
    pub logo_src: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub struct PageRecipient<'a> {
    pub token: &'a str,
    pub role: RecipientRole,
    pub name: &'a str,
}

/// One change request as the page shows it. `at` is the stored timestamp: a
/// UTC `YYYY-MM-DDTHH:MM:SSZ` for an online response, a bare date for one
/// recorded by hand.
#[derive(Debug, Clone)]
pub struct RequestedChange {
    pub name: String,
    pub note: String,
    pub at: String,
}

#[derive(Debug, Clone, Copy)]
pub enum PageState<'a> {
    Open {
        response_url: Option<&'a str>,
    },
    Revising,
    ChangesRequested {
        requests: &'a [RequestedChange],
    },
    Accepted {
        name: &'a str,
        date: &'a str,
    },
    Executed {
        client: (&'a str, &'a str),
        countersign: (&'a str, &'a str),
    },
    Declined,
}

const REPLY_BY_EMAIL: &str = "To respond, reply to the email this link came in.";

const SCRIPT: &str = r#"document.querySelectorAll('form[data-endpoint]').forEach(function (form) {
  form.hidden = false;
  form.addEventListener('submit', function (event) {
    event.preventDefault();
    var d = form.dataset, f = new FormData(form), status = form.querySelector('[data-status]');
    var body = { token: d.token, recipientToken: d.recipient, version: Number(d.version),
                 checksum: d.checksum, action: d.action };
    if (d.action === 'accept') { body.typedName = f.get('typedName'); body.consent = f.get('consent') === 'on'; }
    else { body.note = f.get('note'); }
    form.querySelectorAll('button').forEach(function (b) { b.disabled = true; });
    fetch(d.endpoint, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) })
      .then(function (r) { return r.json().catch(function () { return {}; }).then(function (j) { return { ok: r.ok, j: j }; }); })
      .then(function (res) {
        if (res.ok) { document.querySelectorAll('form[data-endpoint]').forEach(function (x) { x.hidden = true; });
                      var received = document.getElementById('received');
                      received.textContent = received.dataset[d.action === 'accept' ? 'accept' : 'requestChanges'] || received.textContent;
                      received.hidden = false; }
        else { status.textContent = (res.j && res.j.message) || 'This response could not be recorded.';
               form.querySelectorAll('button').forEach(function (b) { b.disabled = false; }); }
      })
      .catch(function () { status.textContent = 'This response could not be sent. Check your connection and try again.';
                           form.querySelectorAll('button').forEach(function (b) { b.disabled = false; }); });
  });
});"#;

const STYLE: &str = "*,*::before,*::after{box-sizing:border-box}[hidden]{display:none!important}\
body{font-family:system-ui,sans-serif;max-width:52rem;margin:2.5rem auto;padding:0 1.25rem;color:#111;line-height:1.5;background:#fff}\
.letterhead{display:flex;align-items:center;justify-content:space-between;gap:1rem;padding-bottom:1.25rem;border-bottom:1px solid #909090;margin-bottom:2rem}\
.logo{max-width:8.8rem;max-height:2.5rem}\
.company,.eyebrow{margin:0;font-size:.75rem;text-transform:uppercase;letter-spacing:.06em;color:#666}\
.eyebrow{margin-bottom:.25rem}\
h1{margin:0 0 1.25rem;font-size:1.75rem;line-height:1.2}\
table.meta{border-collapse:collapse;margin-bottom:1.75rem}\
table.meta th,table.meta td{padding:.2rem 1.5rem .2rem 0;text-align:left;vertical-align:top;font-weight:400}\
table.meta th{color:#666;white-space:nowrap}\
.mono{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:.8rem;word-break:break-all;color:#444}\
.doc{margin:0 0 .75rem;border:1px solid #909090;border-radius:.4rem;overflow:hidden;background:#f4f4f4}\
.doc object{display:block;width:100%;height:75vh}\
.doc object p{padding:2rem;text-align:center}\
.download{margin:0 0 2.5rem}\
.note{font-size:.875rem;color:#444;overflow-wrap:anywhere;border-left:3px solid #909090;padding:.25rem 0 .25rem .9rem;margin:0 0 1.5rem}\
.forms{display:grid;grid-template-columns:repeat(auto-fit,minmax(18rem,1fr));gap:1.25rem;align-items:start}\
.card{border:1px solid #d0d0d0;border-radius:.5rem;padding:1.25rem 1.25rem 1rem}\
.card h2{margin:0 0 .75rem;font-size:1.05rem}\
label{display:block;margin:0 0 .9rem;font-size:.9rem;color:#333}\
input[type=text],textarea{display:block;width:100%;margin-top:.35rem;padding:.55rem .7rem;font:inherit;color:#111;border:1px solid #909090;border-radius:.4rem;background:#fff}\
input[type=text]:focus,textarea:focus{outline:2px solid #111;outline-offset:1px}\
textarea{resize:vertical;min-height:7rem}\
.check{display:flex;gap:.5rem;align-items:flex-start}.check input{margin-top:.25rem}\
.button,button{display:inline-block;padding:.6rem 1.1rem;font:inherit;font-weight:600;border-radius:.4rem;border:1px solid #111;cursor:pointer;text-decoration:none}\
.primary{background:#111;color:#fff}.secondary{background:#fff;color:#111}\
button:disabled{opacity:.5;cursor:default}\
.status{margin:.75rem 0 0;font-size:.875rem;color:#a40000}.status:empty{display:none}\
.notice{padding:.9rem 1.1rem;border-radius:.5rem;background:#f4f4f4;border:1px solid #d0d0d0}\
.notice p{margin:0}.notice p+p{margin-top:.6rem}.request{white-space:pre-wrap;overflow-wrap:anywhere}\
.success{background:#eef7ee;border-color:#9cc79c}\
@media (max-width:36rem){body{margin-top:1.5rem}h1{font-size:1.4rem}.doc object{height:60vh}}\
@media print{.respond,.download{display:none}}";

pub fn relative_pdf_href(version: i64) -> String {
    format!("../v{version}/document.pdf")
}

fn page_shell(title: &str, body: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
<meta name=\"robots\" content=\"noindex\"><title>{}</title><style>{STYLE}</style></head>\
<body>\n{body}\n</body></html>\n",
        esc(title)
    )
}

fn data_attrs(
    ctx: &PageContext<'_>,
    recipient: &PageRecipient<'_>,
    endpoint: &str,
    action: &str,
) -> String {
    format!(
        "data-endpoint=\"{}\" data-token=\"{}\" data-recipient=\"{}\" data-version=\"{}\" data-checksum=\"{}\" data-action=\"{}\"",
        esc(endpoint),
        esc(ctx.token),
        esc(recipient.token),
        ctx.version,
        esc(ctx.checksum),
        action
    )
}

const SIGNATURE_INTENT: &str = "Your typed name is your electronic signature on this document.";

fn signing_paragraph(role: RecipientRole, checksum: &str) -> String {
    match role {
        RecipientRole::Signer => format!(
            "<p class=\"note\">Accepting records your typed name, the time, your IP address and browser against this exact file (checksum {checksum}). {SIGNATURE_INTENT}</p>\n"
        ),
        RecipientRole::Collaborator => String::new(),
    }
}

fn forms(ctx: &PageContext<'_>, recipient: &PageRecipient<'_>, endpoint: &str) -> String {
    let mut out = String::from("<div class=\"forms\">\n");
    if recipient.role == RecipientRole::Signer {
        out.push_str(&format!(
            "<form method=\"post\" hidden {} class=\"card\"><h2>Accept</h2>\
<label>Your full name <input type=\"text\" name=\"typedName\" required maxlength=\"200\" autocomplete=\"name\"></label>\
<label class=\"check\"><input type=\"checkbox\" name=\"consent\" required> I agree to sign electronically</label>\
<button type=\"submit\" class=\"primary\">Accept</button><p data-status role=\"status\" class=\"status\"></p></form>\n",
            data_attrs(ctx, recipient, endpoint, "accept")
        ));
    }
    out.push_str(&format!(
        "<form method=\"post\" hidden {} class=\"card\"><h2>Request changes</h2>\
<label>What should change? <textarea name=\"note\" rows=\"5\" maxlength=\"4000\" required></textarea></label>\
<button type=\"submit\" class=\"secondary\">Request changes</button><p data-status role=\"status\" class=\"status\"></p></form>\n</div>\n\
<p id=\"received\" class=\"notice success\" hidden data-accept=\"Thank you, your acceptance has been received.\" data-request-changes=\"Thank you, your request has been received.\">Thank you, your response has been received.</p>\n<script>{SCRIPT}</script>\n\
<noscript><p>{REPLY_BY_EMAIL}</p></noscript>\n",
        data_attrs(ctx, recipient, endpoint, "request_changes")
    ));
    out
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Rewrites each `<time data-local>` into the reader's own time zone. The
/// server-rendered text, in UTC, is what a reader without script sees.
const LOCAL_TIME_SCRIPT: &str = r#"document.querySelectorAll('time[data-local]').forEach(function (t) {
  var d = new Date(t.getAttribute('datetime'));
  if (isNaN(d)) return;
  t.textContent = d.toLocaleDateString(undefined, { month: 'long', day: 'numeric' }) + ' at ' +
    d.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' });
});"#;

/// `October 9` for a date, and a `<time>` reading `October 9 at 8:44pm UTC`
/// for an online timestamp, which the page localizes. `None` when `at` is
/// neither, so the sentence drops its date rather than printing a raw value.
fn when(at: &str) -> Option<(String, bool)> {
    let month: usize = at.get(5..7)?.parse().ok()?;
    let day: u32 = at.get(8..10)?.parse().ok()?;
    let date = format!("{} {day}", MONTHS.get(month.checked_sub(1)?)?);
    if at.len() == 10 {
        return Some((date, false));
    }
    let hour: u32 = at.get(11..13)?.parse().ok()?;
    let minute = at.get(14..16)?;
    let (h12, half) = match hour {
        0 => (12, "am"),
        1..=11 => (hour, "am"),
        12 => (12, "pm"),
        _ => (hour - 12, "pm"),
    };
    Some((
        format!(
            "<time datetime=\"{}\" data-local>{date} at {h12}:{minute}{half} UTC</time>",
            esc(at)
        ),
        true,
    ))
}

fn changes_requested(requests: &[RequestedChange]) -> String {
    let mut out = String::from(
        "<div class=\"notice\"><p><strong>Changes have been requested. A new document will be sent.</strong></p>\n",
    );
    let mut timed = false;
    for request in requests {
        let on = match when(&request.at) {
            Some((text, is_time)) => {
                timed |= is_time;
                format!(" on {text}")
            }
            None => String::new(),
        };
        out.push_str(&format!(
            "<p>{} said <q class=\"request\">{}</q>{on}.</p>\n",
            esc(&request.name),
            esc(&request.note)
        ));
    }
    out.push_str("</div>\n");
    if timed {
        out.push_str(&format!("<script>{LOCAL_TIME_SCRIPT}</script>\n"));
    }
    out
}

fn state_section(
    ctx: &PageContext<'_>,
    recipient: &PageRecipient<'_>,
    state: &PageState<'_>,
) -> String {
    match state {
        PageState::Open {
            response_url: Some(url),
        } => forms(ctx, recipient, url),
        PageState::Open { response_url: None } => format!("<p class=\"notice\">{REPLY_BY_EMAIL}</p>\n"),
        PageState::Revising => {
            "<p class=\"notice\">This document is being revised. A new version will be sent to you.</p>\n".into()
        }
        PageState::ChangesRequested { requests } => changes_requested(requests),
        PageState::Accepted { name, date } => {
            format!("<p class=\"notice success\">Accepted by {} on {}.</p>\n", esc(name), esc(date))
        }
        PageState::Executed {
            client,
            countersign,
        } => format!(
            "<p class=\"notice success\">Accepted by {} on {}.<br>Countersigned by {} on {}.</p>\n",
            esc(client.0),
            esc(client.1),
            esc(countersign.0),
            esc(countersign.1)
        ),
        PageState::Declined => "<p class=\"notice\">This document was declined.</p>\n".into(),
    }
}

pub fn render_recipient_page(
    ctx: &PageContext<'_>,
    recipient: &PageRecipient<'_>,
    state: &PageState<'_>,
) -> String {
    let body = format!(
        "<header class=\"letterhead\">{logo}<p class=\"company\">{company}</p></header>\n\
<p class=\"eyebrow\">{kind} &middot; Version {version}</p>\n<h1>{title}</h1>\n\
<table class=\"meta\"><tr><th>Prepared for</th><td>{name}</td></tr>\
<tr><th>Client</th><td>{client}</td></tr>\
<tr><th>Checksum</th><td class=\"mono\">{checksum}</td></tr></table>\n\
<div class=\"doc\"><object data=\"{href}\" type=\"application/pdf\"><p><a href=\"{href}\">Download the PDF</a></p></object></div>\n\
<p class=\"download\"><a class=\"button secondary\" href=\"{href}\">Download the PDF</a></p>\n\
<section class=\"respond\">\n{signing}{section}</section>",
        company = esc(ctx.company),
        title = esc(ctx.title),
        kind = esc(ctx.kind),
        client = esc(ctx.client_name),
        version = ctx.version,
        signing = signing_paragraph(recipient.role, &esc(ctx.checksum)),
        logo = ctx
            .logo_src
            .map(|src| format!("<img class=\"logo\" src=\"{}\" alt=\"\">", esc(src)))
            .unwrap_or_default(),
        name = esc(recipient.name),
        href = esc(ctx.pdf_href),
        checksum = esc(ctx.checksum),
        section = state_section(ctx, recipient, state),
    );
    page_shell(ctx.title, &body)
}

pub fn render_document_pages(
    ctx: &PageContext<'_>,
    recipients: &[PageRecipient<'_>],
    state: &PageState<'_>,
) -> Vec<(String, String)> {
    recipients
        .iter()
        .map(|r| (r.token.to_string(), render_recipient_page(ctx, r, state)))
        .collect()
}

pub fn withdrawn_page_html(company: &str, title: &str) -> String {
    page_shell(
        title,
        &format!(
            "<header><p class=\"meta\">{}</p><h1>{}</h1></header>\n<p>This document has been withdrawn.</p>\n",
            esc(company),
            esc(title)
        ),
    )
}

pub fn email_subject(company: &str, title: &str, role: RecipientRole) -> String {
    let ask = match role {
        RecipientRole::Signer => "Please review and sign",
        RecipientRole::Collaborator => "Please review",
    };
    if company.is_empty() {
        format!("{ask}: {title}")
    } else {
        format!("{ask}: {title} for {company}")
    }
}

pub fn render_document_email_text(
    ctx: &PageContext<'_>,
    recipient: &PageRecipient<'_>,
    url: &str,
) -> String {
    let ask = match recipient.role {
        RecipientRole::Signer => "Please review the attached document and respond using the link below.",
        RecipientRole::Collaborator => "The attached document is shared for your review. You can request changes using the link below.",
    };
    format!(
        "Hello {name},\n\n{ask}\n\n{title} (version {version})\n{url}\n",
        name = recipient.name,
        title = ctx.title,
        version = ctx.version,
    )
}

pub fn attachment_name(title: &str, version: i64) -> String {
    let mut stem = String::new();
    let mut pending_dash = false;
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            if pending_dash && !stem.is_empty() {
                stem.push('-');
            }
            pending_dash = false;
            stem.push(c);
        } else {
            pending_dash = true;
        }
    }
    stem.truncate(80);
    let stem = stem.trim_end_matches('-');
    let stem = if stem.is_empty() { "document" } else { stem };
    format!("{stem}-v{version}.pdf")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documents::model::RecipientRole;

    fn ctx() -> PageContext<'static> {
        PageContext {
            company: "Initech",
            client_name: "Cedar Systems",
            kind: "Proposal",
            title: "Website rebuild",
            token: "doc",
            version: 2,
            checksum: "sha256:ab",
            pdf_href: "../v2/document.pdf",
            logo_src: None,
        }
    }
    const SIGNER: PageRecipient<'static> = PageRecipient {
        token: "rs",
        role: RecipientRole::Signer,
        name: "Pat Example",
    };
    const COLLAB: PageRecipient<'static> = PageRecipient {
        token: "rc",
        role: RecipientRole::Collaborator,
        name: "Sam Example",
    };
    const URL: Option<&str> = Some("https://docs.example.test/d/respond");

    /// The pages the Worker's contract test runs the inline script on, so the
    /// body the page posts is checked against what `parseRequest` reads.
    /// `NIGEL_UPDATE_WORKER_FIXTURE=1` rewrites the checked-in copy.
    #[test]
    fn the_worker_contract_fixture_matches_the_rendered_pages() {
        let (doc, signer, collaborator) = (
            "0123456789abcdef0123456789abcdef",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        );
        let ctx = PageContext {
            token: doc,
            ..ctx()
        };
        let recipients = [
            PageRecipient {
                token: signer,
                ..SIGNER
            },
            PageRecipient {
                token: collaborator,
                ..COLLAB
            },
        ];
        let pages =
            render_document_pages(&ctx, &recipients, &PageState::Open { response_url: URL });

        let accept = pages[0].1.split("<form").nth(1).expect("the accept form");
        for attr in [
            format!("data-token=\"{doc}\""),
            format!("data-recipient=\"{signer}\""),
            "data-version=\"2\"".to_string(),
            "data-checksum=\"sha256:ab\"".to_string(),
            "data-action=\"accept\"".to_string(),
        ] {
            assert!(accept.contains(&attr), "{attr}");
        }

        let fixture = serde_json::json!({
            "token": doc,
            "version": ctx.version,
            "checksum": ctx.checksum,
            "recipients": recipients.iter().map(|r| serde_json::json!({
                "token": r.token, "role": r.role.as_str(), "name": r.name,
            })).collect::<Vec<_>>(),
            "pages": pages.iter().map(|(token, html)| serde_json::json!({
                "recipientToken": token, "html": html,
            })).collect::<Vec<_>>(),
        });
        let rendered = format!("{}\n", serde_json::to_string_pretty(&fixture).unwrap());
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../workers/document-response/test/fixtures/page-contract.json");
        if std::env::var_os("NIGEL_UPDATE_WORKER_FIXTURE").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &rendered).unwrap();
        }
        let checked_in = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            checked_in == rendered,
            "{} is stale: rerun this test with NIGEL_UPDATE_WORKER_FIXTURE=1",
            path.display()
        );
    }

    #[test]
    fn a_signer_page_carries_both_forms_and_a_collaborator_page_one() {
        let signer = render_recipient_page(&ctx(), &SIGNER, &PageState::Open { response_url: URL });
        assert!(
            signer.contains("data-action=\"accept\"")
                && signer.contains("data-action=\"request_changes\"")
        );
        let collab = render_recipient_page(&ctx(), &COLLAB, &PageState::Open { response_url: URL });
        assert!(
            !collab.contains("data-action=\"accept\"")
                && collab.contains("data-action=\"request_changes\"")
        );
        for page in [&signer, &collab] {
            assert!(page.contains("<meta name=\"robots\" content=\"noindex\">"));
            assert!(page.contains("sha256:ab") && page.contains("../v2/document.pdf"));
            assert!(page.contains(
                "<noscript><p>To respond, reply to the email this link came in.</p></noscript>"
            ));
            assert_eq!(
                page.matches("<form method=\"post\" hidden data-endpoint=")
                    .count(),
                page.matches("<form").count()
            );
        }
        assert!(signer.contains("I agree to sign electronically"));
        assert!(signer.contains(
            "against this exact file (checksum sha256:ab). Your typed name is your electronic signature on this document."
        ));
        assert!(!collab.contains("electronic signature"));
        assert_eq!(signer.matches("<form").count(), 2);
    }

    #[test]
    fn without_a_response_endpoint_there_is_no_form() {
        let page = render_recipient_page(&ctx(), &SIGNER, &PageState::Open { response_url: None });
        assert!(!page.contains("<form"));
        assert!(page.contains("reply to the email this link came in"));
    }

    #[test]
    fn closed_states_carry_no_form() {
        for state in [
            PageState::Revising,
            PageState::ChangesRequested { requests: &[] },
            PageState::Declined,
            PageState::Accepted {
                name: "Pat Example",
                date: "2026-10-06",
            },
            PageState::Executed {
                client: ("Pat Example", "2026-10-06"),
                countersign: ("Sam Example", "2026-10-07"),
            },
        ] {
            assert!(
                !render_recipient_page(&ctx(), &SIGNER, &state).contains("<form"),
                "{state:?}"
            );
        }
        let accepted = render_recipient_page(
            &ctx(),
            &SIGNER,
            &PageState::Accepted {
                name: "Pat Example",
                date: "2026-10-06",
            },
        );
        assert!(accepted.contains("Accepted by Pat Example on 2026-10-06"));
    }

    #[test]
    fn a_note_or_name_with_markup_is_escaped_on_the_page() {
        let evil = PageRecipient {
            name: "<img src=x onerror=alert(1)>",
            ..SIGNER
        };
        let page = render_recipient_page(
            &PageContext {
                title: "<script>x</script>",
                ..ctx()
            },
            &evil,
            &PageState::Accepted {
                name: "<b>Pat</b>",
                date: "2026-10-06",
            },
        );
        assert!(
            !page.contains("<img src=x") && !page.contains("<script>x") && !page.contains("<b>Pat")
        );
        assert!(page.contains("&lt;b&gt;Pat&lt;/b&gt;"));
    }

    #[test]
    fn attribute_values_cannot_break_out() {
        let ctx = PageContext {
            token: "a\"><script>1</script>",
            checksum: "x\" onload=\"y",
            ..ctx()
        };
        let page = render_recipient_page(
            &ctx,
            &SIGNER,
            &PageState::Open {
                response_url: Some("https://e.test/\"><i>"),
            },
        );
        assert!(
            !page.contains("<script>1") && !page.contains("<i>") && !page.contains("onload=\"y")
        );
    }

    #[test]
    fn subjects_and_attachment_names() {
        assert_eq!(
            email_subject("Initech", "Website rebuild", RecipientRole::Signer),
            "Please review and sign: Website rebuild for Initech"
        );
        assert_eq!(
            email_subject("", "Website rebuild", RecipientRole::Collaborator),
            "Please review: Website rebuild"
        );
        assert_eq!(
            attachment_name("Website rebuild / Phase 2", 2),
            "Website-rebuild-Phase-2-v2.pdf"
        );
        assert_eq!(attachment_name("—", 1), "document-v1.pdf");
        assert_eq!(
            attachment_name(&"a".repeat(200), 3),
            format!("{}-v3.pdf", "a".repeat(80))
        );
    }

    #[test]
    fn the_email_body_is_plain_text_with_the_personal_link() {
        let text = render_document_email_text(
            &ctx(),
            &SIGNER,
            "https://docs.example.test/d/doc/rs/index.html",
        );
        assert!(text.contains("https://docs.example.test/d/doc/rs/index.html"));
        assert!(!text.contains('<'));
        assert!(text.ends_with("/d/doc/rs/index.html\n"));
        assert!(!text.contains("electronic signature"));
    }

    #[test]
    fn a_collaborator_email_ends_with_the_link() {
        let text = render_document_email_text(
            &ctx(),
            &COLLAB,
            "https://docs.example.test/d/doc/rc/index.html",
        );
        assert!(text.ends_with("/d/doc/rc/index.html\n"));
    }

    #[test]
    fn one_page_per_recipient() {
        let pages = render_document_pages(&ctx(), &[SIGNER, COLLAB], &PageState::Revising);
        assert_eq!(
            pages.iter().map(|p| p.0.as_str()).collect::<Vec<_>>(),
            ["rs", "rc"]
        );
        assert!(!pages[1].1.contains("\"rs\""));
        let notice = |state| render_recipient_page(&ctx(), &SIGNER, &state);
        assert!(notice(PageState::Revising)
            .contains("This document is being revised. A new version will be sent to you."));
        let requests = [
            RequestedChange {
                name: "Pat Example".into(),
                note: "Strike paragraph 3 <and> add a cancellation clause.".into(),
                at: "2026-10-09T23:44:51Z".into(),
            },
            RequestedChange {
                name: "Sam Example".into(),
                note: "Also fix the dates.".into(),
                at: "2026-10-10".into(),
            },
        ];
        let changes = notice(PageState::ChangesRequested {
            requests: &requests,
        });
        assert!(changes.contains("Changes have been requested. A new document will be sent."));
        assert!(changes.contains(
            "<p>Pat Example said <q class=\"request\">Strike paragraph 3 &lt;and&gt; add a cancellation clause.</q> on <time datetime=\"2026-10-09T23:44:51Z\" data-local>October 9 at 11:44pm UTC</time>.</p>"
        ));
        assert!(changes.contains(
            "<p>Sam Example said <q class=\"request\">Also fix the dates.</q> on October 10.</p>"
        ));
        assert!(changes.contains("time[data-local]") && !changes.contains("<form"));
        assert!(notice(PageState::Declined).contains("This document was declined."));
        let executed = notice(PageState::Executed {
            client: ("Pat Example", "2026-10-06"),
            countersign: ("Sam Example", "2026-10-07"),
        });
        assert!(
            executed.contains("Accepted by Pat Example on 2026-10-06")
                && executed.contains("Countersigned by Sam Example on 2026-10-07")
        );
        assert!(withdrawn_page_html("Initech", "<b>x</b>").contains("noindex"));
        assert_eq!(relative_pdf_href(3), "../v3/document.pdf");
    }
}
