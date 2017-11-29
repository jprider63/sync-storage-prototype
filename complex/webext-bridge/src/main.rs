#[macro_use]
extern crate serde_derive;

extern crate serde;
extern crate serde_json;
extern crate toodle;

use std::io::{self, Read};
use std::collections::HashMap;

use toodle::Toodle;

enum Error {
    BadRequest,
    BadToodle,
    BadLabel,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
enum Request {
    NewToodle { uri: String },
    DestroyToodle { toodle_id: i64 },

    GetLabels,
    GetTodos { labels: Option<Vec<String>> },

    CreateTodo {
        name: String,
        due: i64,
        labels: Vec<String>,
    },
    DeleteTodo { id: i64 },
    MarkCompleted { id: i64 },

    CreateLabel {
        toodle_id: i64,
        name: String,
        color: String,
    },
    AddLabel {
        toodle_id: i64,
        item_uuid: String,
        label: Label,
    },
    SetDue { id: i64, due: i64 },
}

impl Request {
    fn read_from(&mut input: BufRead) -> Result<Request, Error> {
        let length = input.read_u32::<NativeEndian>();
        let mut message = input.take(length);
        serde_json::from_reader(message).map_err(|_| Error::BadRequest)
    }
}

#[derive(Serialize, Debug)]
#[serde(tag = "type")]
enum Response {
    NewToodle { toodle_id: i64 },
    DestroyToodle { destroyed: bool },
    CreateLabel { label: Label },
    CreateItem { item: Item },
    AddLabel,

    Err(Error),
}

impl Response {
    fn write_to(&self, output: Write) -> Result<()> {
        let message = serde_json::to_vec(self)?;
        output.write_u32::<NativeEndian>(message.len() as u32)?;
        output.write_all(message)
    }
}

fn main() {
    let mut toodles = HashMap::new::<i64, Toodle>();
    let mut next_toodle_id = 1i64;

    let stdin = io::stdin();
    let stdout = io::stdout();

    let mut input = stdin.lock();
    let mut output = stdout.lock();

    loop {
        let response = match Request::read_from(input) {
            Ok(request) => {
                match request {
                    Request::NewToodle { uri } => {
                        let toodle = Toodle::new(uri);
                        toodles.insert(next_toodle_id, toodle);
                        let response = Response::NewToodle { toodle_id: next_toodle_id };
                        next_toodle_id += 1;
                        response
                    }
                    Request::DestroyToodle { toodle_id } => {
                        let maybe_toodle = toodles.remove(toodle_id);
                        Response::DestroyToodle { destroyed: maybe_toodle.is_some() }
                    }
                    Request::CreateLabel {
                        toodle_id,
                        name,
                        color,
                    } => {
                        match toodles.get(toodle_id) {
                            Some(toodle) => {
                                match toodle.store.create_label(name, color) {
                                    Some(label) => Response::CreateLabel { label },
                                    None => Response::Err(BadLabel),
                                }
                            }
                            None => Response::Err(BadToodle),
                        }
                    }
                    Request::AddLabel {
                        toodle_id,
                        item_uuid,
                        label,
                    } => {
                        match toodles.get(toodle_id) {
                            Some(toodle) => {
                                match toodle.store.fetch_item(item_uuid) {
                                    Some(mut item) => {
                                        if !item.labels.contains(label) {
                                            item.labels.push(label);
                                            toodle.store.update_item(&item);
                                        }
                                        Response::AddLabel
                                    }
                                    None => Response::Err(BadItem),
                                }
                            }
                            None => Response::Err(BadToodle),
                        }
                    }
                }
            }
            Err(err) => Response::Error(err),
        };
        response
            .write_to(output)
            .unwrap_or_else(|err| {
                                eprintln!("Error handling request: {:?}", err);
                            });
    }
}

impl Serialize for Color {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        let mut state = serializer.serialize_struct("Label", 2)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("label", &self.label)?;
        state.end()
    }
}

impl Serialize for Item {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        let mut state = serializer.serialize_struct("Item", 5)?;
        state.serialize_field("uuid", &self.uuid)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("due_date", &self.due_date)?;
        state
            .serialize_field("completion_date", &self.completion_date)?;
        state.serialize_field("labels", &self.labels)?;
        state.end()
    }
}
