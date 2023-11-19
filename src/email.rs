use crate::Equipment;
use clap::Args;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{message::MultiPart, Message, SmtpTransport, Transport};
use std::error::Error;
use tera::Tera;

#[derive(Args, Debug)]
pub struct EMailArgs {
    /// E-Mail Address to send the status to
    #[clap(long)]
    status_address: String,

    /// E-Mail Address to send errors to (if any)
    #[clap(long)]
    errors_address: String,

    /// smtp server address
    #[clap(long)]
    smtp_server: String,

    /// smtp username
    #[clap(long)]
    smtp_user: String,

    /// smtp password
    #[clap(long)]
    smtp_password: String,
}

pub fn send_result(
    equipments: &Vec<Equipment>,
    errors: &Vec<Box<dyn Error>>,
    tera: &Tera,
    args: &EMailArgs,
) {
    let num_ok = equipments
        .iter()
        .filter(|eq| eq.working.unwrap_or_default())
        .count();

    let num_failed = equipments
        .iter()
        .filter(|eq| !eq.working.unwrap_or(true))
        .count();

    let num_unknown = equipments.iter().filter(|eq| eq.working.is_none()).count() + errors.len();

    let ok_status = if num_ok > 0 { "✅" } else { "" };
    let failed_status = if num_failed > 0 { "⛔" } else { "" };
    let unknown_status = if num_unknown > 0 { "❔" } else { "" };

    let status_message = match (num_failed, num_ok, num_unknown) {
        (0, _, _) if num_ok > 0 && num_unknown > 0 => "Kein defekter Aufzug (einige Unbekannt)!",
        (0, 0, _) => "Warnung: Aufzugstatus unbekannt!",
        (0, _, 0) => "Alle Aufzüge funktionieren!",
        (_, _, _) => "Achtung: Defekter Aufzug auf dem Weg!",
    };

    let mut context = tera::Context::new();
    context.insert("equipments", equipments);
    context.insert(
        "errors",
        &errors.iter().map(|err| err.to_string()).collect::<Vec<_>>(),
    );
    let html_message = tera
        .render("status.html", &context)
        .unwrap_or_else(|err| format!("Error while creating message: {}", err));
    let text_message = tera
        .render("status.txt", &context)
        .unwrap_or_else(|err| format!("Error while creating message: {}", err));

    let email = Message::builder()
        .from(format!("ElStatus <{}>", args.smtp_user).parse().unwrap())
        .to(args.status_address.parse().unwrap())
        .subject(format!(
            "{}{}{} {}",
            failed_status, ok_status, unknown_status, status_message
        ))
        .multipart(MultiPart::alternative_plain_html(
            text_message,
            html_message,
        ))
        .unwrap();

    let creds = Credentials::new(args.smtp_user.clone(), args.smtp_password.clone());

    let mailer = SmtpTransport::relay(&args.smtp_server)
        .unwrap()
        .credentials(creds)
        .build();

    // Send the email
    match mailer.send(&email) {
        Ok(_) => println!("Status email sent successfully!"),
        Err(e) => panic!("Could not send email: {:?}", e),
    }
}

pub fn send_errors(errors: &Vec<Box<dyn Error>>, tera: &Tera, args: &EMailArgs) {
    if errors.is_empty() {
        return;
    }

    let mut context = tera::Context::new();
    context.insert(
        "errors",
        &errors.iter().map(|err| err.to_string()).collect::<Vec<_>>(),
    );
    let text_message = tera
        .render("errors.txt", &context)
        .unwrap_or_else(|err| format!("Error while creating message: {}", err.to_string()));

    let email = Message::builder()
        .from(format!("ElStatus <{}>", args.smtp_user).parse().unwrap())
        .to(args.errors_address.parse().unwrap())
        .subject(format!(
            "{} Errors encountered when checking elevator status",
            errors.len()
        ))
        .body(text_message)
        .unwrap();

    let creds = Credentials::new(args.smtp_user.clone(), args.smtp_password.clone());

    let mailer = SmtpTransport::relay(&*args.smtp_server)
        .unwrap()
        .credentials(creds)
        .build();

    // Send the email
    match mailer.send(&email) {
        Ok(_) => println!("Errors E-Mail sent successfully!"),
        Err(e) => panic!("Could not send email: {:?}", e),
    }
}
