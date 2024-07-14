use std::ops::ControlFlow;

use crate::{
    take_cf_content, DeserializeError, Deserr, ErrorKind, IntoValue, Map, Sequence, Value,
    ValueKind, ValuePointerRef,
};
use serde_yml::{Mapping as YMap, Number, Sequence as YSeq, Value as YValue};

pub struct YMapIter {
    iter: <YMap as IntoIterator>::IntoIter,
}

impl Iterator for YMapIter {
    type Item = (String, YValue);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(k, v)| {
            match k {
                // kinda questionable but oh well
                YValue::String(s) => (s, v),
                _ => panic!(),
            }
        })
    }
}

impl Map for YMap {
    type Value = YValue;
    type Iter = YMapIter;

    fn len(&self) -> usize {
        self.len()
    }
    fn remove(&mut self, key: &str) -> Option<Self::Value> {
        self.remove(key)
    }
    fn into_iter(self) -> Self::Iter {
        YMapIter {
            iter: <Self as IntoIterator>::into_iter(self),
        }
    }
}

impl IntoValue for YValue {
    type Sequence = YSeq;
    type Map = YMap;

    fn into_value(self) -> Value<Self> {
        match self {
            YValue::Null => Value::Null,
            YValue::Bool(b) => Value::Boolean(b),
            YValue::Number(n) => {
                if let Some(n) = n.as_u64() {
                    Value::Integer(n)
                } else if let Some(n) = n.as_i64() {
                    Value::NegativeInteger(n)
                } else if let Some(n) = n.as_f64() {
                    Value::Float(n)
                } else {
                    panic!();
                }
            }
            YValue::String(x) => Value::String(x),
            YValue::Sequence(x) => Value::Sequence(x),
            YValue::Mapping(x) => Value::Map(x),
            // TODO do what serde_yml does and make this an enum discriminant?
            YValue::Tagged(x) => x.value.into_value(),
        }
    }

    fn kind(&self) -> ValueKind {
        match self {
            YValue::Null => ValueKind::Null,
            YValue::Bool(_) => ValueKind::Boolean,
            YValue::Number(n) => {
                if n.is_u64() {
                    ValueKind::Integer
                } else if n.is_i64() {
                    ValueKind::NegativeInteger
                } else if n.is_f64() {
                    ValueKind::Float
                } else {
                    panic!();
                }
            }
            YValue::String(_) => ValueKind::String,
            YValue::Sequence(_) => ValueKind::Sequence,
            YValue::Mapping(_) => ValueKind::Map,
            // TODO see above
            YValue::Tagged(x) => x.value.kind(),
        }
    }
}

impl<E: DeserializeError> Deserr<E> for YValue {
    fn deserialize_from_value<V: IntoValue>(
        value: Value<V>,
        location: ValuePointerRef,
    ) -> Result<Self, E> {
        let mut error: Option<E> = None;
        Ok(match value {
            Value::Null => YValue::Null,
            Value::Boolean(b) => YValue::Bool(b),
            Value::Integer(x) => YValue::Number(Number::from(x)),
            Value::NegativeInteger(x) => YValue::Number(Number::from(x)),
            Value::Float(f) => YValue::Number(Number::from(f)),
            Value::String(s) => YValue::String(s),
            Value::Sequence(seq) => {
                let mut yseq = Vec::with_capacity(seq.len());
                for (index, value) in seq.into_iter().enumerate() {
                    let result = Self::deserialize_from_value(
                        value.into_value(),
                        location.push_index(index),
                    );
                    match result {
                        Ok(value) => {
                            yseq.push(value);
                        }
                        Err(e) => {
                            error = match E::merge(error, e, location.push_index(index)) {
                                ControlFlow::Continue(e) => Some(e),
                                ControlFlow::Break(e) => return Err(e),
                            };
                        }
                    }
                }
                if let Some(e) = error {
                    return Err(e);
                } else {
                    YValue::Sequence(yseq)
                }
            }
            Value::Map(map) => {
                let mut jmap = YMap::with_capacity(map.len());
                for (key, value) in map.into_iter() {
                    let result =
                        Self::deserialize_from_value(value.into_value(), location.push_key(&key));
                    match result {
                        Ok(value) => {
                            jmap.insert(YValue::String(key), value);
                        }
                        Err(e) => {
                            error = match E::merge(error, e, location.push_key(&key)) {
                                ControlFlow::Continue(e) => Some(e),
                                ControlFlow::Break(e) => return Err(e),
                            };
                        }
                    }
                }
                if let Some(e) = error {
                    return Err(e);
                } else {
                    YValue::Mapping(jmap)
                }
            }
        })
    }
}

impl<V: IntoValue> From<Value<V>> for YValue {
    fn from(value: Value<V>) -> Self {
        match value {
            Value::Null => YValue::Null,
            Value::Boolean(b) => YValue::Bool(b),
            Value::Integer(n) => YValue::Number(Number::from(n)),
            Value::NegativeInteger(i) => YValue::Number(Number::from(i)),
            // if we can't parse the float then its set to `null`
            Value::Float(f) => YValue::Number(Number::from(f)),
            Value::String(s) => YValue::String(s),
            Value::Sequence(s) => YValue::Sequence(
                s.into_iter()
                    .map(IntoValue::into_value)
                    .map(YValue::from)
                    .collect(),
            ),
            Value::Map(m) => YValue::Mapping(YMap::from_iter(
                m.into_iter()
                    .map(|(k, v)| (YValue::String(k), YValue::from(v.into_value()))),
            )),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_json::json;

    #[test]
    fn from_value_to_deserr_and_back() {
        let value: YValue = serde_yml::from_str("
        The: best
        doggos: [are]
        the:
          bernese: mountain").unwrap();
        let deserr = value.clone().into_value();

        insta::assert_debug_snapshot!(deserr, @r###"
        Map(
            Mapping {
                "The": String("best"),
                "doggos": Sequence [
                    String("are"),
                ],
                "the": Mapping {
                    "bernese": String("mountain"),
                },
            },
        )
        "###);

        let deserr: YValue = deserr.into();
        insta::assert_debug_snapshot!(deserr, @r###"
        Mapping {
            "The": String("best"),
            "doggos": Sequence [
                String("are"),
            ],
            "the": Mapping {
                "bernese": String("mountain"),
            },
        }
        "###);

        assert_eq!(value, deserr);
    }
}
