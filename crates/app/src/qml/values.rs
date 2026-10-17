use cxx_qt_lib::{QList, QMap, QMapPair_QString_QVariant, QMetaTypeType, QString, QVariant};
use rimstack_core::base::sentence::{MARK, Sentence};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Map, Number, Value};

pub fn to_qml(value: &impl Serialize) -> QVariant {
    from_value(&as_shown(value))
}

pub fn as_shown(value: &impl Serialize) -> Value {
    fn shown(value: Value) -> Value {
        match value {
            Value::Object(fields) if fields.len() == 1 && fields.contains_key(MARK) => {
                Value::from(crate::words::put(&Sentence::from_kept_value(&fields[MARK])))
            }
            Value::Object(fields) => {
                Value::Object(fields.into_iter().map(|(k, v)| (k, shown(v))).collect())
            }
            Value::Array(items) => Value::Array(items.into_iter().map(shown).collect()),
            other => other,
        }
    }
    match serde_json::to_value(value) {
        Ok(value) => shown(value),
        Err(problem) => {
            rimstack_core::problem!("not given to the window: {problem}");
            Value::Null
        }
    }
}

/// `None` for what does not read as `T`.
pub fn from_qml<T: DeserializeOwned>(given: &QVariant) -> Option<T> {
    serde_json::from_value(to_value(given))
        .map_err(|problem| rimstack_core::problem!("not read from the window: {problem}"))
        .ok()
}

/// `None` for what is not a list of ids; an empty list is `Some`.
pub fn mod_ids(given: &QVariant) -> Option<Vec<rimstack_core::mods::library::ModId>> {
    let held: Vec<i32> = from_qml(given)?;
    Some(
        held.into_iter()
            .filter_map(rimstack_core::mods::library::ModId::from_qml)
            .collect(),
    )
}

fn from_value(value: &Value) -> QVariant {
    match value {
        Value::Null => QVariant::default(),
        Value::Bool(b) => QVariant::from(b),
        Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => QVariant::from(&i),
            (None, Some(f)) => QVariant::from(&f),
            (None, None) => QVariant::default(),
        },
        Value::String(text) => QVariant::from(&QString::from(text.as_str())),
        Value::Array(items) => {
            let mut list = QList::<QVariant>::default();
            for item in items {
                list.append(from_value(item));
            }
            QVariant::from(&list)
        }
        Value::Object(fields) => {
            let mut map = QMap::<QMapPair_QString_QVariant>::default();
            for (name, field) in fields {
                map.insert(QString::from(name.as_str()), from_value(field));
            }
            QVariant::from(&map)
        }
    }
}

/// JavaScript has only one kind of number: an id QML holds is a double.
fn whole_or_not(n: f64) -> Value {
    const EXACT: f64 = 9_007_199_254_740_992.0;
    if n.fract() == 0.0 && n.abs() <= EXACT {
        Value::from(n as i64)
    } else {
        Number::from_f64(n).map_or(Value::Null, Value::Number)
    }
}

fn to_value(given: &QVariant) -> Value {
    let number = |n: Option<Number>| n.map_or(Value::Null, Value::Number);
    match given.type_id() {
        QMetaTypeType::Bool => Value::Bool(given.value_or_default::<bool>()),
        QMetaTypeType::Int => Value::from(given.value_or_default::<i32>()),
        QMetaTypeType::UInt => Value::from(given.value_or_default::<u32>()),
        QMetaTypeType::Short | QMetaTypeType::Long | QMetaTypeType::LongLong => {
            Value::from(given.value_or_default::<i64>())
        }
        QMetaTypeType::UShort | QMetaTypeType::ULong | QMetaTypeType::ULongLong => {
            Value::from(given.value_or_default::<u64>())
        }
        QMetaTypeType::Double => whole_or_not(given.value_or_default::<f64>()),
        QMetaTypeType::Float => number(Number::from_f64(given.value_or_default::<f32>().into())),
        QMetaTypeType::QString => Value::String(given.value_or_default::<QString>().to_string()),
        QMetaTypeType::QStringList => {
            let texts = given.value_or_default::<cxx_qt_lib::QStringList>();
            let list: QList<QString> = QList::from(&texts);
            Value::Array(list.iter().map(|t| Value::String(t.to_string())).collect())
        }
        QMetaTypeType::QVariantList => {
            let list = given.value_or_default::<QList<QVariant>>();
            Value::Array(list.iter().map(to_value).collect())
        }
        QMetaTypeType::QVariantMap => {
            let map = given.value_or_default::<QMap<QMapPair_QString_QVariant>>();
            let fields: Map<String, Value> = map
                .iter()
                .map(|(name, field)| (name.to_string(), to_value(field)))
                .collect();
            Value::Object(fields)
        }
        _ => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, serde::Deserialize, Debug, PartialEq)]
    #[serde(rename_all = "camelCase")]
    struct Fix {
        action: String,
    }

    #[derive(Serialize, serde::Deserialize, Debug, PartialEq)]
    #[serde(rename_all = "camelCase")]
    struct Problem {
        mod_id: i64,
        error: bool,
        megabytes: Option<f64>,
        fix: Fix,
        tags: Vec<String>,
    }

    #[test]
    fn a_record_goes_to_the_window_and_comes_back_the_same() {
        let sent = vec![Problem {
            mod_id: 3,
            error: true,
            megabytes: Some(1.5),
            fix: Fix {
                action: "trust".into(),
            },
            tags: vec!["roads".into(), "é".into()],
        }];
        let given = to_qml(&sent);
        assert_eq!(given.type_id(), QMetaTypeType::QVariantList);
        assert_eq!(from_qml::<Vec<Problem>>(&given), Some(sent));
    }

    #[test]
    fn nothing_given_and_the_wrong_shape_read_as_none() {
        assert_eq!(from_qml::<Vec<i64>>(&QVariant::default()), None);
        assert_eq!(from_qml::<Vec<i64>>(&to_qml(&"text")), None);
        assert_eq!(from_qml::<Option<String>>(&QVariant::default()), Some(None));
    }

    #[test]
    fn a_malformed_list_of_ids_is_refused_and_an_empty_one_is_taken() {
        assert_eq!(mod_ids(&to_qml(&Vec::<i32>::new())), Some(Vec::new()));
        assert_eq!(mod_ids(&to_qml(&"text")), None);
        assert_eq!(mod_ids(&QVariant::default()), None);
        assert_eq!(mod_ids(&to_qml(&vec!["a", "b"])), None);
        assert_eq!(mod_ids(&to_qml(&vec![3, 7])).map(|ids| ids.len()), Some(2));
    }

    #[test]
    fn a_malformed_list_of_keys_is_refused_and_an_empty_one_is_taken() {
        assert_eq!(
            from_qml::<Vec<String>>(&to_qml(&Vec::<String>::new())),
            Some(Vec::new())
        );
        assert_eq!(from_qml::<Vec<String>>(&to_qml(&"text")), None);
        assert_eq!(from_qml::<Vec<String>>(&to_qml(&vec![1, 2])), None);
    }

    #[test]
    fn numbers_keep_their_kind() {
        assert_eq!(from_qml::<i64>(&to_qml(&-7_i64)), Some(-7));
        assert_eq!(to_qml(&0.25_f64).type_id(), QMetaTypeType::Double);
        assert_eq!(from_qml::<f64>(&to_qml(&0.25_f64)), Some(0.25));
        assert_eq!(
            from_qml::<u64>(&to_qml(&3_000_000_000_u64)),
            Some(3_000_000_000)
        );
        assert_eq!(from_qml::<i32>(&QVariant::from(&5_i32)), Some(5));
    }

    #[test]
    fn ids_held_by_javascript_as_doubles_read_as_whole_numbers() {
        let mut list = QList::<QVariant>::default();
        list.append(QVariant::from(&3.0_f64));
        list.append(QVariant::from(&7.0_f64));
        assert_eq!(
            from_qml::<Vec<i32>>(&QVariant::from(&list)),
            Some(vec![3, 7])
        );
        assert_eq!(from_qml::<f64>(&QVariant::from(&0.5_f64)), Some(0.5));
    }
}
