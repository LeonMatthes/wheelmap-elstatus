use lazy_static::lazy_static;
use serde::Serialize;
use serde_json::Value;
use std::error::Error;

const ACCESS_TOKEN: &str = "YOUR_ACCESS_TOKEN";

struct EquipmentList {
    latitude: f32,
    longitude: f32,
    equipment_searches: Vec<&'static str>,
}

lazy_static! {
    static ref EQUIPMENTS: Vec<EquipmentList> = vec! [
        // Berlin-Wannsee
        EquipmentList {
            latitude: 52.422206,
            longitude: 13.1810253,
            equipment_searches: vec![
                "Gleis 1/2",
                "Gleis 3/4",
                "EG Tunnel",
            ]
        },
        EquipmentList {
            latitude: 52.394356,
            longitude: 13.127521,
            equipment_searches: vec![
                "Gleis 1/2",
                "Gleis 5"
            ]
        },
        EquipmentList {
            latitude: 52.443315,
            longitude: 13.293771,
            equipment_searches: vec![
                "Gleis 1/2 (S-Bahn)",
                "EG Tunnel"
            ]
        }
    ];
}

#[derive(Debug)]
enum EquipmentAccessError {
    MissingValue(String, String),
    InvalidType {
        expected_type: String,
        json: String,
    },
    HTTPRequestError {
        status: reqwest::StatusCode,
        response_text: String,
    },
}

impl std::fmt::Display for EquipmentAccessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EquipmentAccessError::MissingValue(value, json) => {
                write!(f, "Missing value: {} in JSON: {}", value, json)
            }
            EquipmentAccessError::InvalidType {
                expected_type,
                json,
            } => {
                write!(
                    f,
                    "Expected JSON: {} to be of type: {}",
                    json, expected_type
                )
            }
            EquipmentAccessError::HTTPRequestError {
                status,
                response_text,
            } => {
                write!(
                    f,
                    "HTTP request failed, error code: {}\n{}",
                    status.as_str(),
                    response_text
                )
            }
        }
    }
}

impl Error for EquipmentAccessError {}

#[derive(Debug, Serialize, Clone)]
struct Equipment {
    name: String,
    working: Option<bool>,
    place: Option<String>,
}

fn parse_equipment(json: &Value) -> Result<Equipment, EquipmentAccessError> {
    if let Some(properties) = &json.get("properties") {
        let working = properties
            .get("isWorking")
            .unwrap_or(&Value::default())
            .as_bool();
        let name = properties
            .get("description")
            .and_then(|description| description.get("de").unwrap_or(description).as_str())
            .unwrap_or("Cannot find description!")
            .to_owned();
        let place = properties
            .get("placeInfoName")
            .and_then(Value::as_str)
            .map(str::to_owned);

        Ok(Equipment {
            name,
            working,
            place,
        })
    } else {
        Err(EquipmentAccessError::MissingValue(
            "description".to_owned(),
            json.to_string(),
        ))
    }
}

fn parse_equipment_list(json: &Value) -> Result<Vec<Equipment>, Vec<EquipmentAccessError>> {
    if let Some(equipments) = json.as_array() {
        let (equipments, errors): (Vec<_>, _) = equipments
            .iter()
            .map(parse_equipment)
            .partition(Result::is_ok);

        let equipments: Vec<Equipment> = equipments.into_iter().filter_map(Result::ok).collect();
        let errors = errors.into_iter().filter_map(Result::err).collect();

        if equipments.is_empty() {
            Err(errors)
        } else {
            Ok(equipments)
        }
    } else {
        Err(vec![EquipmentAccessError::InvalidType {
            expected_type: "Array".to_string(),
            json: json.to_string(),
        }])
    }
}

fn get_equipments(list: &EquipmentList) -> Result<Vec<Equipment>, Box<dyn Error>> {
    let request = reqwest::blocking::get(format!(
        "https://accessibility-cloud.freetls.fastly.net/equipment-infos.json?appToken={}&latitude={}&longitude={}&accuracy=500",
        ACCESS_TOKEN, list.latitude, list.longitude
    ))?;

    if !request.status().is_success() {
        return Err(EquipmentAccessError::HTTPRequestError {
            status: request.status(),
            response_text: request.text().unwrap_or("No text received!".to_owned()),
        }
        .into());
    }

    let json_string = request.text()?;
    let json: Value = serde_json::from_str(&*json_string)?;

    if let Some(features) = json.get("features") {
        let equipments = parse_equipment_list(features);

        match equipments {
            Ok(source_equipments) => {
                let mut corpus = ngrammatic::CorpusBuilder::new().finish();
                for equipment in &source_equipments {
                    corpus.add_text(&*equipment.name);
                }

                let mut results = Vec::new();

                for search in &list.equipment_searches {
                    let query_result = corpus.search(search, 0.4);

                    if let Some(result_name) = query_result.first() {
                        if let Some(equipment) = source_equipments
                            .iter()
                            .find(|equipment| equipment.name == result_name.text)
                        {
                            results.push(equipment.clone());
                        }
                    }
                }

                return Ok(results);
            }
            Err(errors) => {
                let errors_string: String = errors
                    .iter()
                    .map(EquipmentAccessError::to_string)
                    .fold(String::new(), |a, b| a + "\n" + &*b);
                return Err(format!(
                    "Errors encountered when sourcing equipments:\n{}",
                    errors_string
                )
                .into());
            }
        }
    }
    Err(Box::new(EquipmentAccessError::MissingValue(
        "".to_owned(),
        json_string,
    )))
}

use lettre::transport::smtp::authentication::Credentials;
use lettre::{message::MultiPart, Message, SmtpTransport, Transport};

use tera::Tera;

fn send_result(
    equipments: &Vec<Equipment>,
    errors: &Vec<Box<dyn Error>>,
    tera: &Tera,
    args: &Args,
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
        (_, _, _) => "Achtung: Defekter Aufzug auf Arbeitsweg!",
    };

    let mut context = tera::Context::new();
    context.insert("equipments", equipments);
    context.insert(
        "errors",
        &errors.iter().map(|err| err.to_string()).collect::<Vec<_>>(),
    );
    let html_message = tera
        .render("status.html", &context)
        .unwrap_or_else(|err| format!("Error while creating message: {}", err.to_string()));
    let text_message = tera
        .render("status.txt", &context)
        .unwrap_or_else(|err| format!("Error while creating message: {}", err.to_string()));

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

    let mailer = SmtpTransport::relay(&*args.smtp_server)
        .unwrap()
        .credentials(creds)
        .build();

    // Send the email
    match mailer.send(&email) {
        Ok(_) => println!("Status email sent successfully!"),
        Err(e) => panic!("Could not send email: {:?}", e),
    }
}

fn send_errors(errors: &Vec<Box<dyn Error>>, tera: &Tera, args: &Args) {
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

use clap::Parser;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
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

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let mut tera = Tera::default();
    tera.add_raw_templates(vec![
        ("status.html", include_str!("templates/status.html")),
        ("status.txt", include_str!("templates/status.txt")),
        ("errors.txt", include_str!("templates/errors.txt")),
    ])?;

    let (equipments, errors): (Vec<_>, Vec<_>) = EQUIPMENTS
        .iter()
        .map(get_equipments)
        .partition(Result::is_ok);

    let equipments: Vec<_> = equipments.into_iter().flat_map(Result::unwrap).collect();
    let errors: Vec<_> = errors
        .into_iter()
        .map(Result::err)
        .map(Option::unwrap)
        .collect();

    for equipment in equipments.iter() {
        println!("{:?}", equipment);
    }

    for error in errors.iter() {
        println!("Error: {}", error);
    }

    send_result(&equipments, &errors, &tera, &args);

    send_errors(&errors, &tera, &args);

    Ok(())
}
