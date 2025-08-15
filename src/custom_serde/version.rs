use crate::config::AdaptunerVersion;

const VERSION: &str = env!("CARGO_PKG_VERSION");

impl serde::Serialize for AdaptunerVersion {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(VERSION)
    }
}

impl<'de> serde::Deserialize<'de> for AdaptunerVersion {
    fn deserialize<D: serde::de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let version_str = <&'de str as serde::Deserialize<'de>>::deserialize(deserializer)?;
        if version_str == VERSION {
            Ok(AdaptunerVersion)
        } else {
            Err(serde::de::Error::custom(format!(
                "version mismatch: this is adaptuner version {VERSION}, but \
                the configuration file is for {version_str}"
            )))
        }
    }
}
