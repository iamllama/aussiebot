use serde::{ser::Serialize, Deserialize, Deserializer, Serializer};

macro_rules! impl_serde_bitflags {
    ($($name:ident),+$(,)?) => {
      $(
        impl Serialize for $name {
          fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
          where
              S: Serializer,
          {
              serializer.serialize_u32(self.bits())
          }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let bits = u32::deserialize(deserializer)?;
                $name::from_bits(bits)
                    .ok_or(format!(concat!("Unable to deserialise ",stringify!($name),": invalid bit flags {:?}"), bits))
                    .map_err(serde::de::Error::custom)
            }
        }
      )+
    };
}

use super::Permissions;
use super::Platform;

impl_serde_bitflags!(Platform, Permissions);
