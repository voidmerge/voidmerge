use crate::types::BoxFut;
use bytes::Bytes;
use std::collections::HashMap;
use std::io::Result;
use std::sync::Arc;

/// Helper to transform a value tree.
pub trait ValueTx: Send {
    /// Transform a unit value.
    fn unit(&mut self) -> BoxFut<'_, Result<Value>> {
        Box::pin(async move { Ok(Value::Unit) })
    }

    /// Transform a boolean value.
    fn bool(&mut self, b: bool) -> BoxFut<'_, Result<Value>> {
        Box::pin(async move { Ok(Value::Bool(b)) })
    }

    /// Transform a float value.
    fn float(&mut self, f: f64) -> BoxFut<'_, Result<Value>> {
        Box::pin(async move { Ok(Value::Float(f)) })
    }

    /// Transform a str value.
    fn str(&mut self, s: Arc<str>) -> BoxFut<'_, Result<Value>> {
        Box::pin(async move { Ok(Value::Str(s)) })
    }

    /// Transform a bytes value.
    fn bytes(&mut self, b: Bytes) -> BoxFut<'_, Result<Value>> {
        Box::pin(async move { Ok(Value::Bytes(b)) })
    }
}

/// Convert a [Value] tree with possible binary data into "human" mode
/// where binary data is now `{{b64-bin <data>}}` strings.
#[derive(Default, Debug)]
pub struct ValueTxToHuman;

impl ValueTx for ValueTxToHuman {
    fn str(&mut self, s: Arc<str>) -> BoxFut<'_, Result<Value>> {
        use base64::prelude::*;

        Box::pin(async move {
            if s.trim().starts_with("{{") {
                let tmp = BASE64_URL_SAFE_NO_PAD.encode(s.as_bytes());
                let tmp = format!("{{{{b64-str {tmp}}}}}");
                Ok(Value::Str(tmp.into()))
            } else {
                Ok(Value::Str(s))
            }
        })
    }

    fn bytes(&mut self, b: Bytes) -> BoxFut<'_, Result<Value>> {
        use base64::prelude::*;

        Box::pin(async move {
            let tmp = BASE64_URL_SAFE_NO_PAD.encode(&b);
            let tmp = format!("{{{{b64-bin {tmp}}}}}");
            Ok(Value::Str(tmp.into()))
        })
    }
}

/// Convert a [Value] tree with template tags into a [Value] tree where those are parsed into literal values.
///
/// - `{{inc-bin <file>}}`
/// - `{{inc-str <file>}}`
/// - `{{b64-bin <data>}}`
/// - `{{b64-str <data>}}`
#[derive(Debug)]
pub struct ValueTxFromHuman {
    root: std::path::PathBuf,
}

impl ValueTxFromHuman {
    /// Construct a new transform instance.
    pub fn new(root: impl AsRef<std::path::Path>) -> Self {
        Self {
            root: root.as_ref().into(),
        }
    }
}

impl ValueTx for ValueTxFromHuman {
    fn str(&mut self, s: Arc<str>) -> BoxFut<'_, Result<Value>> {
        use base64::prelude::*;

        Box::pin(async move {
            let tmp = s.trim();
            if !tmp.starts_with("{{") || !tmp.ends_with("}}") {
                return Ok(Value::Str(s));
            }
            let tmp =
                tmp.trim_start_matches("{{").trim_end_matches("}}").trim();
            let (cmd, rest) = tmp.split_once(" ").ok_or_else(|| {
                std::io::Error::other(format!("invalid template command {s}"))
            })?;
            match cmd.trim() {
                cmd @ "inc-bin" | cmd @ "inc-str" => {
                    let file = self.root.join(rest.trim());
                    let data = tokio::fs::read(file).await?;
                    if cmd == "inc-str" {
                        let data = String::from_utf8_lossy(&data);
                        Ok(Value::Str(data.into()))
                    } else {
                        Ok(Value::Bytes(data.into()))
                    }
                }
                cmd @ "b64-bin" | cmd @ "b64-str" => {
                    let data = BASE64_URL_SAFE_NO_PAD
                        .decode(rest.trim())
                        .map_err(std::io::Error::other)?;
                    if cmd == "b64-str" {
                        let data = String::from_utf8_lossy(&data);
                        Ok(Value::Str(data.into()))
                    } else {
                        Ok(Value::Bytes(data.into()))
                    }
                }
                oth => Err(std::io::Error::other(format!(
                    "unrecognized template cmd: {oth}"
                ))),
            }
        })
    }
}

/// Generic serde compatible [Value] type.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Empty type.
    Unit,

    /// Boolean type.
    Bool(bool),

    /// Floating-point number type.
    Float(f64),

    /// Utf8 string type.
    Str(Arc<str>),

    /// Byte array type.
    Bytes(Bytes),

    /// Array type.
    Array(Vec<Box<Self>>),

    /// Map type.
    Map(HashMap<Arc<str>, Box<Self>>),
}

impl From<()> for Value {
    fn from(_: ()) -> Self {
        Self::Unit
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Self::Float(f)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Self::Str(s.into())
    }
}

impl From<Arc<str>> for Value {
    fn from(s: Arc<str>) -> Self {
        Self::Str(s)
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Self::Str(s.into())
    }
}

impl From<Bytes> for Value {
    fn from(b: Bytes) -> Self {
        Self::Bytes(b)
    }
}

impl Value {
    /// Construct a new empty [Value::Array].
    pub fn array_new() -> Self {
        Self::Array(Default::default())
    }

    /// Get a sub-item if this value is a [Value::Array].
    pub fn array_get(&self, i: usize) -> Option<&Self> {
        match self {
            Self::Array(a) => a.get(i).map(|v| &**v),
            _ => None,
        }
    }

    /// Push a new item onto this, if this is a [Value::Array].
    pub fn array_push(&mut self, v: Self) {
        if let Self::Array(a) = self {
            a.push(Box::new(v));
        }
    }

    /// Construct a new empty [Value::Map].
    pub fn map_new() -> Self {
        Self::Map(Default::default())
    }

    /// Get a sub-item if this value is a [Value::Map].
    pub fn map_get(&self, k: &str) -> Option<&Self> {
        match self {
            Self::Map(m) => m.get(k).map(|v| &**v),
            _ => None,
        }
    }

    /// Remove and return a sub-item if this value is a [Value::Map].
    pub fn map_remove(&mut self, k: &str) -> Option<Self> {
        match self {
            Self::Map(m) => m.remove(k).map(|v| *v),
            _ => None,
        }
    }

    /// Insert an item into this, if this is a [Value::Map].
    pub fn map_insert(&mut self, k: Arc<str>, v: Self) {
        if let Self::Map(m) = self {
            m.insert(k, Box::new(v));
        }
    }

    /// Get this item if it is a [Value::Bool].
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Get this item if it is a [Value::Float].
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// Get this item if it is a [Value::Str].
    pub fn as_string(&self) -> Option<&Arc<str>> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }

    /// Get this item if it is a [Value::Bytes].
    pub fn as_bytes(&self) -> Option<&Bytes> {
        match self {
            Self::Bytes(b) => Some(b),
            _ => None,
        }
    }

    /// Get this item if it is a [Value::Array].
    pub fn as_array(&self) -> Option<&Vec<Box<Self>>> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Get this item if it is a [Value::Map].
    pub fn as_map(&self) -> Option<&HashMap<Arc<str>, Box<Self>>> {
        match self {
            Self::Map(m) => Some(m),
            _ => None,
        }
    }

    /// Transform this value tree.
    pub async fn transform(self, tx: &mut impl ValueTx) -> Result<Self> {
        fn rec<'a>(
            tx: &'a mut impl ValueTx,
            value: Value,
        ) -> BoxFut<'a, Result<Value>> {
            Box::pin(async move {
                match value {
                    Value::Unit => tx.unit().await,
                    Value::Bool(b) => tx.bool(b).await,
                    Value::Float(f) => tx.float(f).await,
                    Value::Str(s) => tx.str(s).await,
                    Value::Bytes(b) => tx.bytes(b).await,
                    Value::Array(a) => {
                        let mut out = Vec::with_capacity(a.len());
                        for v in a {
                            out.push(Box::new(rec(tx, *v).await?));
                        }
                        Ok(Value::Array(out))
                    }
                    Value::Map(m) => {
                        let mut out = HashMap::with_capacity(m.len());
                        for (k, v) in m {
                            out.insert(k, Box::new(rec(tx, *v).await?));
                        }
                        Ok(Value::Map(out))
                    }
                }
            })
        }
        rec(tx, self).await
    }
}

impl serde::Serialize for Value {
    #[inline]
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Value::Unit => serializer.serialize_unit(),
            Value::Bool(b) => serializer.serialize_bool(*b),
            Value::Float(f) => serializer.serialize_f64(*f),
            Value::Str(s) => serializer.serialize_str(s),
            Value::Bytes(b) => serializer.serialize_bytes(&b[..]),
            Value::Array(v) => v.serialize(serializer),
            Value::Map(m) => {
                use serde::ser::SerializeMap;
                let mut map = serializer.serialize_map(Some(m.len()))?;
                for (k, v) in m {
                    map.serialize_entry(k, v)?;
                }
                map.end()
            }
        }
    }
}

impl<'de> serde::Deserialize<'de> for Value {
    #[inline]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ValueVisitor;

        impl<'de> serde::de::Visitor<'de> for ValueVisitor {
            type Value = Value;

            fn expecting(
                &self,
                formatter: &mut std::fmt::Formatter,
            ) -> std::fmt::Result {
                formatter.write_str("any valid JSON value")
            }

            #[inline]
            fn visit_bool<E>(
                self,
                value: bool,
            ) -> std::result::Result<Value, E> {
                Ok(Value::Bool(value))
            }

            fn visit_byte_buf<E>(
                self,
                v: Vec<u8>,
            ) -> std::result::Result<Value, E> {
                Ok(Value::Bytes(v.into()))
            }

            fn visit_bytes<E>(self, v: &[u8]) -> std::result::Result<Value, E> {
                Ok(Value::Bytes(Bytes::copy_from_slice(v)))
            }

            #[inline]
            fn visit_i64<E>(self, value: i64) -> std::result::Result<Value, E> {
                Ok(Value::Float(value as f64))
            }

            #[inline]
            fn visit_u64<E>(self, value: u64) -> std::result::Result<Value, E> {
                Ok(Value::Float(value as f64))
            }

            #[inline]
            fn visit_f64<E>(self, value: f64) -> std::result::Result<Value, E> {
                Ok(Value::Float(value))
            }

            #[inline]
            fn visit_str<E>(self, value: &str) -> std::result::Result<Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Value::Str(value.into()))
            }

            #[inline]
            fn visit_string<E>(
                self,
                value: String,
            ) -> std::result::Result<Value, E> {
                Ok(Value::Str(value.into()))
            }

            #[inline]
            fn visit_none<E>(self) -> std::result::Result<Value, E> {
                Ok(Value::Unit)
            }

            #[inline]
            fn visit_some<D>(
                self,
                deserializer: D,
            ) -> std::result::Result<Value, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                serde::Deserialize::deserialize(deserializer)
            }

            #[inline]
            fn visit_unit<E>(self) -> std::result::Result<Value, E> {
                Ok(Value::Unit)
            }

            #[inline]
            fn visit_seq<V>(
                self,
                mut visitor: V,
            ) -> std::result::Result<Value, V::Error>
            where
                V: serde::de::SeqAccess<'de>,
            {
                let mut vec = Vec::new();

                while let Some(elem) = visitor.next_element()? {
                    vec.push(elem);
                }

                Ok(Value::Array(vec))
            }

            fn visit_map<V>(
                self,
                mut visitor: V,
            ) -> std::result::Result<Value, V::Error>
            where
                V: serde::de::MapAccess<'de>,
            {
                let mut map: HashMap<Arc<str>, Box<Value>> = Default::default();

                while let Some((k, v)) = visitor.next_entry()? {
                    map.insert(k, v);
                }

                Ok(Value::Map(map))
            }
        }

        deserializer.deserialize_any(ValueVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_unit_round() {
        let orig =
            serde_json::from_str(&serde_json::to_string(&Value::Unit).unwrap())
                .unwrap();
        assert!(matches!(orig, Value::Unit));
    }

    #[test]
    fn value_bool_round() {
        let orig = serde_json::from_str(
            &serde_json::to_string(&Value::Bool(true)).unwrap(),
        )
        .unwrap();
        assert!(matches!(orig, Value::Bool(b) if b));
    }

    #[test]
    fn value_float_round() {
        let orig = serde_json::from_str(
            &serde_json::to_string(&Value::Float(3.141)).unwrap(),
        )
        .unwrap();
        assert!(matches!(orig, Value::Float(f) if f == 3.141));
    }

    #[test]
    fn value_str_round() {
        let orig = serde_json::from_str(
            &serde_json::to_string(&Value::Str("hello".into())).unwrap(),
        )
        .unwrap();
        assert!(matches!(orig, Value::Str(s) if &*s == "hello"));
    }

    #[tokio::test]
    async fn tx_to_human() {
        let mut a = Value::array_new();
        a.array_push(Value::Str("{{ not a real template".into()));
        a.array_push(Value::Bytes((&b"hello"[..]).into()));

        let mut m1 = Value::map_new();
        m1.map_insert("m1".into(), Value::Str("{{ fake".into()));
        a.array_push(m1);

        let mut m2 = Value::map_new();
        m2.map_insert("m2".into(), Value::Bytes((&b"world"[..]).into()));
        a.array_push(m2);

        let b = a
            .clone()
            .transform(&mut ValueTxToHuman::default())
            .await
            .unwrap();

        let json = serde_json::to_string(&b).unwrap();

        let mut expect = String::new();
        expect.push_str("[\"{{b64-str e3sgbm90IGEgcmVhbCB0ZW1wbGF0ZQ}}\",");
        expect.push_str("\"{{b64-bin aGVsbG8}}\",");
        expect.push_str("{\"m1\":\"{{b64-str e3sgZmFrZQ}}\"},");
        expect.push_str("{\"m2\":\"{{b64-bin d29ybGQ}}\"}]");
        assert_eq!(expect, json);

        let c = b.transform(&mut ValueTxFromHuman::new(".")).await.unwrap();
        assert_eq!(a, c);
    }
}
