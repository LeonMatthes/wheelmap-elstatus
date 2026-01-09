use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;

pub mod display;
pub mod email;

#[derive(Debug)]
pub enum EquipmentAccessError {
    MissingValue(String, String),
    InvalidType {
        expected_type: String,
        json: String,
    },
    HTTPRequestError {
        status: reqwest::StatusCode,
        response_text: String,
    },
    CannotFindEquipment {
        query_text: String,
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
            EquipmentAccessError::CannotFindEquipment { query_text } => {
                write!(f, "Could not find elevator: {}", query_text)
            }
        }
    }
}

impl Error for EquipmentAccessError {}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct Equipment {
    name: String,
    category: String,
    working: Option<bool>,
    place: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct EquipmentList {
    pub latitude: f32,
    pub longitude: f32,
    pub equipment_searches: Vec<String>,
}

pub fn get_equipments(list: &EquipmentList) -> Result<Vec<Equipment>, Box<dyn Error>> {
    let access_token = std::env::var("WHEELMAP_TOKEN")?;
    let request = reqwest::blocking::get(format!(
        "https://accessibility-cloud.freetls.fastly.net/equipment-infos.json?appToken={}&latitude={}&longitude={}&accuracy=500",
        &access_token, list.latitude, list.longitude
    ))?;

    if !request.status().is_success() {
        return Err(EquipmentAccessError::HTTPRequestError {
            status: request.status(),
            response_text: request.text().unwrap_or("No text received!".to_owned()),
        }
        .into());
    }

    let json_string = request.text()?;
    let json: Value = serde_json::from_str(&json_string)?;

    if let Some(features) = json.get("features") {
        let equipments = parse_equipment_list(features);

        match equipments {
            Ok(source_equipments) => {
                let mut corpus = ngrammatic::CorpusBuilder::new().finish();
                for equipment in &source_equipments {
                    corpus.add_text(&equipment.name);
                }

                let mut results = Vec::new();

                for search in &list.equipment_searches {
                    let query_result = corpus.search(&search, 0.4);

                    if let Some(equipment) = query_result.first().and_then(|result_name| {
                        source_equipments
                            .iter()
                            .find(|equipment| equipment.name == result_name.text)
                    }) {
                        results.push(equipment.clone());
                    } else {
                        return Err(Box::new(EquipmentAccessError::CannotFindEquipment {
                            query_text: search.to_owned(),
                        }));
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
        let category = properties
            .get("category")
            .and_then(Value::as_str)
            .unwrap_or("elevator")
            .to_owned();
        let place = properties
            .get("placeInfoName")
            .and_then(Value::as_str)
            .map(str::to_owned);

        Ok(Equipment {
            name,
            category,
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

        let equipments: Vec<Equipment> = equipments
            .into_iter()
            .filter_map(Result::ok)
            // reject any escalators or other
            // unknown equipment which may
            // have the same name
            .filter(|equipment| equipment.category.to_lowercase() == "elevator")
            .collect();
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
