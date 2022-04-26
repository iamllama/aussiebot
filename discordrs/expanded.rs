#![feature(prelude_import)]
#[prelude_import]
use std::prelude::rust_2021::*;
#[macro_use]
extern crate std;
mod msg {
    use serde_derive::{Deserialize, Serialize};
    pub enum Platform {
        Broadcast,
        Youtube,
        Discord,
        Twitch,
        Web,
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::fmt::Debug for Platform {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match (&*self,) {
                (&Platform::Broadcast,) => ::core::fmt::Formatter::write_str(f, "Broadcast"),
                (&Platform::Youtube,) => ::core::fmt::Formatter::write_str(f, "Youtube"),
                (&Platform::Discord,) => ::core::fmt::Formatter::write_str(f, "Discord"),
                (&Platform::Twitch,) => ::core::fmt::Formatter::write_str(f, "Twitch"),
                (&Platform::Web,) => ::core::fmt::Formatter::write_str(f, "Web"),
            }
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::clone::Clone for Platform {
        #[inline]
        fn clone(&self) -> Platform {
            {
                *self
            }
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::marker::Copy for Platform {}
    impl ::core::marker::StructuralPartialEq for Platform {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::PartialEq for Platform {
        #[inline]
        fn eq(&self, other: &Platform) -> bool {
            {
                let __self_vi = ::core::intrinsics::discriminant_value(&*self);
                let __arg_1_vi = ::core::intrinsics::discriminant_value(&*other);
                if true && __self_vi == __arg_1_vi {
                    match (&*self, &*other) {
                        _ => true,
                    }
                } else {
                    false
                }
            }
        }
    }
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl _serde::Serialize for Platform {
            fn serialize<__S>(
                &self,
                __serializer: __S,
            ) -> _serde::__private::Result<__S::Ok, __S::Error>
            where
                __S: _serde::Serializer,
            {
                match *self {
                    Platform::Broadcast => _serde::Serializer::serialize_unit_variant(
                        __serializer,
                        "Platform",
                        0u32,
                        "Broadcast",
                    ),
                    Platform::Youtube => _serde::Serializer::serialize_unit_variant(
                        __serializer,
                        "Platform",
                        1u32,
                        "Youtube",
                    ),
                    Platform::Discord => _serde::Serializer::serialize_unit_variant(
                        __serializer,
                        "Platform",
                        2u32,
                        "Discord",
                    ),
                    Platform::Twitch => _serde::Serializer::serialize_unit_variant(
                        __serializer,
                        "Platform",
                        3u32,
                        "Twitch",
                    ),
                    Platform::Web => _serde::Serializer::serialize_unit_variant(
                        __serializer,
                        "Platform",
                        4u32,
                        "Web",
                    ),
                }
            }
        }
    };
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl<'de> _serde::Deserialize<'de> for Platform {
            fn deserialize<__D>(__deserializer: __D) -> _serde::__private::Result<Self, __D::Error>
            where
                __D: _serde::Deserializer<'de>,
            {
                #[allow(non_camel_case_types)]
                enum __Field {
                    __field0,
                    __field1,
                    __field2,
                    __field3,
                    __field4,
                }
                struct __FieldVisitor;
                impl<'de> _serde::de::Visitor<'de> for __FieldVisitor {
                    type Value = __Field;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(__formatter, "variant identifier")
                    }
                    fn visit_u64<__E>(
                        self,
                        __value: u64,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            0u64 => _serde::__private::Ok(__Field::__field0),
                            1u64 => _serde::__private::Ok(__Field::__field1),
                            2u64 => _serde::__private::Ok(__Field::__field2),
                            3u64 => _serde::__private::Ok(__Field::__field3),
                            4u64 => _serde::__private::Ok(__Field::__field4),
                            _ => _serde::__private::Err(_serde::de::Error::invalid_value(
                                _serde::de::Unexpected::Unsigned(__value),
                                &"variant index 0 <= i < 5",
                            )),
                        }
                    }
                    fn visit_str<__E>(
                        self,
                        __value: &str,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            "Broadcast" => _serde::__private::Ok(__Field::__field0),
                            "Youtube" => _serde::__private::Ok(__Field::__field1),
                            "Discord" => _serde::__private::Ok(__Field::__field2),
                            "Twitch" => _serde::__private::Ok(__Field::__field3),
                            "Web" => _serde::__private::Ok(__Field::__field4),
                            _ => _serde::__private::Err(_serde::de::Error::unknown_variant(
                                __value, VARIANTS,
                            )),
                        }
                    }
                    fn visit_bytes<__E>(
                        self,
                        __value: &[u8],
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            b"Broadcast" => _serde::__private::Ok(__Field::__field0),
                            b"Youtube" => _serde::__private::Ok(__Field::__field1),
                            b"Discord" => _serde::__private::Ok(__Field::__field2),
                            b"Twitch" => _serde::__private::Ok(__Field::__field3),
                            b"Web" => _serde::__private::Ok(__Field::__field4),
                            _ => {
                                let __value = &_serde::__private::from_utf8_lossy(__value);
                                _serde::__private::Err(_serde::de::Error::unknown_variant(
                                    __value, VARIANTS,
                                ))
                            }
                        }
                    }
                }
                impl<'de> _serde::Deserialize<'de> for __Field {
                    #[inline]
                    fn deserialize<__D>(
                        __deserializer: __D,
                    ) -> _serde::__private::Result<Self, __D::Error>
                    where
                        __D: _serde::Deserializer<'de>,
                    {
                        _serde::Deserializer::deserialize_identifier(__deserializer, __FieldVisitor)
                    }
                }
                struct __Visitor<'de> {
                    marker: _serde::__private::PhantomData<Platform>,
                    lifetime: _serde::__private::PhantomData<&'de ()>,
                }
                impl<'de> _serde::de::Visitor<'de> for __Visitor<'de> {
                    type Value = Platform;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(__formatter, "enum Platform")
                    }
                    fn visit_enum<__A>(
                        self,
                        __data: __A,
                    ) -> _serde::__private::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::EnumAccess<'de>,
                    {
                        match match _serde::de::EnumAccess::variant(__data) {
                            _serde::__private::Ok(__val) => __val,
                            _serde::__private::Err(__err) => {
                                return _serde::__private::Err(__err);
                            }
                        } {
                            (__Field::__field0, __variant) => {
                                match _serde::de::VariantAccess::unit_variant(__variant) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                };
                                _serde::__private::Ok(Platform::Broadcast)
                            }
                            (__Field::__field1, __variant) => {
                                match _serde::de::VariantAccess::unit_variant(__variant) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                };
                                _serde::__private::Ok(Platform::Youtube)
                            }
                            (__Field::__field2, __variant) => {
                                match _serde::de::VariantAccess::unit_variant(__variant) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                };
                                _serde::__private::Ok(Platform::Discord)
                            }
                            (__Field::__field3, __variant) => {
                                match _serde::de::VariantAccess::unit_variant(__variant) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                };
                                _serde::__private::Ok(Platform::Twitch)
                            }
                            (__Field::__field4, __variant) => {
                                match _serde::de::VariantAccess::unit_variant(__variant) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                };
                                _serde::__private::Ok(Platform::Web)
                            }
                        }
                    }
                }
                const VARIANTS: &'static [&'static str] =
                    &["Broadcast", "Youtube", "Discord", "Twitch", "Web"];
                _serde::Deserializer::deserialize_enum(
                    __deserializer,
                    "Platform",
                    VARIANTS,
                    __Visitor {
                        marker: _serde::__private::PhantomData::<Platform>,
                        lifetime: _serde::__private::PhantomData,
                    },
                )
            }
        }
    };
    impl Default for Platform {
        fn default() -> Self {
            Self::Broadcast
        }
    }
    impl Platform {
        fn from_u64(n: u64) -> Option<Self> {
            match n {
                0 => Some(Platform::Broadcast),
                1 => Some(Platform::Youtube),
                2 => Some(Platform::Discord),
                3 => Some(Platform::Twitch),
                4 => Some(Platform::Web),
                _ => None,
            }
        }
        pub fn from_str(s: impl Into<String>) -> Option<Self> {
            match s.into().to_lowercase().as_ref() {
                "broadcast" => Some(Platform::Broadcast),
                "y" | "yt" | "youtube" => Some(Platform::Youtube),
                "d" | "disc" | "discord" => Some(Platform::Discord),
                "t" | "tw" | "twitch" => Some(Platform::Twitch),
                "web" => Some(Platform::Web),
                _ => None,
            }
        }
    }
    pub enum Permissions {
        None = 0,
        Member = 1,
        Admin = 2,
        Owner = 3,
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::fmt::Debug for Permissions {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match (&*self,) {
                (&Permissions::None,) => ::core::fmt::Formatter::write_str(f, "None"),
                (&Permissions::Member,) => ::core::fmt::Formatter::write_str(f, "Member"),
                (&Permissions::Admin,) => ::core::fmt::Formatter::write_str(f, "Admin"),
                (&Permissions::Owner,) => ::core::fmt::Formatter::write_str(f, "Owner"),
            }
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::PartialOrd for Permissions {
        #[inline]
        fn partial_cmp(
            &self,
            other: &Permissions,
        ) -> ::core::option::Option<::core::cmp::Ordering> {
            {
                let __self_vi = ::core::intrinsics::discriminant_value(&*self);
                let __arg_1_vi = ::core::intrinsics::discriminant_value(&*other);
                if true && __self_vi == __arg_1_vi {
                    match (&*self, &*other) {
                        _ => ::core::option::Option::Some(::core::cmp::Ordering::Equal),
                    }
                } else {
                    ::core::cmp::PartialOrd::partial_cmp(&__self_vi, &__arg_1_vi)
                }
            }
        }
    }
    impl ::core::marker::StructuralPartialEq for Permissions {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::PartialEq for Permissions {
        #[inline]
        fn eq(&self, other: &Permissions) -> bool {
            {
                let __self_vi = ::core::intrinsics::discriminant_value(&*self);
                let __arg_1_vi = ::core::intrinsics::discriminant_value(&*other);
                if true && __self_vi == __arg_1_vi {
                    match (&*self, &*other) {
                        _ => true,
                    }
                } else {
                    false
                }
            }
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::clone::Clone for Permissions {
        #[inline]
        fn clone(&self) -> Permissions {
            {
                *self
            }
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::marker::Copy for Permissions {}
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl<'de> _serde::Deserialize<'de> for Permissions {
            fn deserialize<__D>(__deserializer: __D) -> _serde::__private::Result<Self, __D::Error>
            where
                __D: _serde::Deserializer<'de>,
            {
                #[allow(non_camel_case_types)]
                enum __Field {
                    __field0,
                    __field1,
                    __field2,
                    __field3,
                }
                struct __FieldVisitor;
                impl<'de> _serde::de::Visitor<'de> for __FieldVisitor {
                    type Value = __Field;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(__formatter, "variant identifier")
                    }
                    fn visit_u64<__E>(
                        self,
                        __value: u64,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            0u64 => _serde::__private::Ok(__Field::__field0),
                            1u64 => _serde::__private::Ok(__Field::__field1),
                            2u64 => _serde::__private::Ok(__Field::__field2),
                            3u64 => _serde::__private::Ok(__Field::__field3),
                            _ => _serde::__private::Err(_serde::de::Error::invalid_value(
                                _serde::de::Unexpected::Unsigned(__value),
                                &"variant index 0 <= i < 4",
                            )),
                        }
                    }
                    fn visit_str<__E>(
                        self,
                        __value: &str,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            "None" => _serde::__private::Ok(__Field::__field0),
                            "Member" => _serde::__private::Ok(__Field::__field1),
                            "Admin" => _serde::__private::Ok(__Field::__field2),
                            "Owner" => _serde::__private::Ok(__Field::__field3),
                            _ => _serde::__private::Err(_serde::de::Error::unknown_variant(
                                __value, VARIANTS,
                            )),
                        }
                    }
                    fn visit_bytes<__E>(
                        self,
                        __value: &[u8],
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            b"None" => _serde::__private::Ok(__Field::__field0),
                            b"Member" => _serde::__private::Ok(__Field::__field1),
                            b"Admin" => _serde::__private::Ok(__Field::__field2),
                            b"Owner" => _serde::__private::Ok(__Field::__field3),
                            _ => {
                                let __value = &_serde::__private::from_utf8_lossy(__value);
                                _serde::__private::Err(_serde::de::Error::unknown_variant(
                                    __value, VARIANTS,
                                ))
                            }
                        }
                    }
                }
                impl<'de> _serde::Deserialize<'de> for __Field {
                    #[inline]
                    fn deserialize<__D>(
                        __deserializer: __D,
                    ) -> _serde::__private::Result<Self, __D::Error>
                    where
                        __D: _serde::Deserializer<'de>,
                    {
                        _serde::Deserializer::deserialize_identifier(__deserializer, __FieldVisitor)
                    }
                }
                struct __Visitor<'de> {
                    marker: _serde::__private::PhantomData<Permissions>,
                    lifetime: _serde::__private::PhantomData<&'de ()>,
                }
                impl<'de> _serde::de::Visitor<'de> for __Visitor<'de> {
                    type Value = Permissions;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(__formatter, "enum Permissions")
                    }
                    fn visit_enum<__A>(
                        self,
                        __data: __A,
                    ) -> _serde::__private::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::EnumAccess<'de>,
                    {
                        match match _serde::de::EnumAccess::variant(__data) {
                            _serde::__private::Ok(__val) => __val,
                            _serde::__private::Err(__err) => {
                                return _serde::__private::Err(__err);
                            }
                        } {
                            (__Field::__field0, __variant) => {
                                match _serde::de::VariantAccess::unit_variant(__variant) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                };
                                _serde::__private::Ok(Permissions::None)
                            }
                            (__Field::__field1, __variant) => {
                                match _serde::de::VariantAccess::unit_variant(__variant) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                };
                                _serde::__private::Ok(Permissions::Member)
                            }
                            (__Field::__field2, __variant) => {
                                match _serde::de::VariantAccess::unit_variant(__variant) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                };
                                _serde::__private::Ok(Permissions::Admin)
                            }
                            (__Field::__field3, __variant) => {
                                match _serde::de::VariantAccess::unit_variant(__variant) {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                };
                                _serde::__private::Ok(Permissions::Owner)
                            }
                        }
                    }
                }
                const VARIANTS: &'static [&'static str] = &["None", "Member", "Admin", "Owner"];
                _serde::Deserializer::deserialize_enum(
                    __deserializer,
                    "Permissions",
                    VARIANTS,
                    __Visitor {
                        marker: _serde::__private::PhantomData::<Permissions>,
                        lifetime: _serde::__private::PhantomData,
                    },
                )
            }
        }
    };
    impl From<u64> for Permissions {
        fn from(p: u64) -> Self {
            match p {
                0 => Permissions::None,
                1 => Permissions::Member,
                2 => Permissions::Admin,
                3 => Permissions::Owner,
                _ => Permissions::None,
            }
        }
    }
    impl Default for Permissions {
        fn default() -> Self {
            Self::None
        }
    }
    pub struct User {
        pub name: String,
        pub id: String,
        pub platform: Platform,
        pub perms: Permissions,
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::fmt::Debug for User {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match *self {
                User {
                    name: ref __self_0_0,
                    id: ref __self_0_1,
                    platform: ref __self_0_2,
                    perms: ref __self_0_3,
                } => {
                    let debug_trait_builder = &mut ::core::fmt::Formatter::debug_struct(f, "User");
                    let _ = ::core::fmt::DebugStruct::field(
                        debug_trait_builder,
                        "name",
                        &&(*__self_0_0),
                    );
                    let _ =
                        ::core::fmt::DebugStruct::field(debug_trait_builder, "id", &&(*__self_0_1));
                    let _ = ::core::fmt::DebugStruct::field(
                        debug_trait_builder,
                        "platform",
                        &&(*__self_0_2),
                    );
                    let _ = ::core::fmt::DebugStruct::field(
                        debug_trait_builder,
                        "perms",
                        &&(*__self_0_3),
                    );
                    ::core::fmt::DebugStruct::finish(debug_trait_builder)
                }
            }
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::default::Default for User {
        #[inline]
        fn default() -> User {
            User {
                name: ::core::default::Default::default(),
                id: ::core::default::Default::default(),
                platform: ::core::default::Default::default(),
                perms: ::core::default::Default::default(),
            }
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::clone::Clone for User {
        #[inline]
        fn clone(&self) -> User {
            match *self {
                User {
                    name: ref __self_0_0,
                    id: ref __self_0_1,
                    platform: ref __self_0_2,
                    perms: ref __self_0_3,
                } => User {
                    name: ::core::clone::Clone::clone(&(*__self_0_0)),
                    id: ::core::clone::Clone::clone(&(*__self_0_1)),
                    platform: ::core::clone::Clone::clone(&(*__self_0_2)),
                    perms: ::core::clone::Clone::clone(&(*__self_0_3)),
                },
            }
        }
    }
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl<'de> _serde::Deserialize<'de> for User {
            fn deserialize<__D>(__deserializer: __D) -> _serde::__private::Result<Self, __D::Error>
            where
                __D: _serde::Deserializer<'de>,
            {
                #[allow(non_camel_case_types)]
                enum __Field {
                    __field0,
                    __field1,
                    __field2,
                    __field3,
                    __ignore,
                }
                struct __FieldVisitor;
                impl<'de> _serde::de::Visitor<'de> for __FieldVisitor {
                    type Value = __Field;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(__formatter, "field identifier")
                    }
                    fn visit_u64<__E>(
                        self,
                        __value: u64,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            0u64 => _serde::__private::Ok(__Field::__field0),
                            1u64 => _serde::__private::Ok(__Field::__field1),
                            2u64 => _serde::__private::Ok(__Field::__field2),
                            3u64 => _serde::__private::Ok(__Field::__field3),
                            _ => _serde::__private::Ok(__Field::__ignore),
                        }
                    }
                    fn visit_str<__E>(
                        self,
                        __value: &str,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            "name" => _serde::__private::Ok(__Field::__field0),
                            "id" => _serde::__private::Ok(__Field::__field1),
                            "platform" => _serde::__private::Ok(__Field::__field2),
                            "perms" => _serde::__private::Ok(__Field::__field3),
                            _ => _serde::__private::Ok(__Field::__ignore),
                        }
                    }
                    fn visit_bytes<__E>(
                        self,
                        __value: &[u8],
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            b"name" => _serde::__private::Ok(__Field::__field0),
                            b"id" => _serde::__private::Ok(__Field::__field1),
                            b"platform" => _serde::__private::Ok(__Field::__field2),
                            b"perms" => _serde::__private::Ok(__Field::__field3),
                            _ => _serde::__private::Ok(__Field::__ignore),
                        }
                    }
                }
                impl<'de> _serde::Deserialize<'de> for __Field {
                    #[inline]
                    fn deserialize<__D>(
                        __deserializer: __D,
                    ) -> _serde::__private::Result<Self, __D::Error>
                    where
                        __D: _serde::Deserializer<'de>,
                    {
                        _serde::Deserializer::deserialize_identifier(__deserializer, __FieldVisitor)
                    }
                }
                struct __Visitor<'de> {
                    marker: _serde::__private::PhantomData<User>,
                    lifetime: _serde::__private::PhantomData<&'de ()>,
                }
                impl<'de> _serde::de::Visitor<'de> for __Visitor<'de> {
                    type Value = User;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(__formatter, "struct User")
                    }
                    #[inline]
                    fn visit_seq<__A>(
                        self,
                        mut __seq: __A,
                    ) -> _serde::__private::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::SeqAccess<'de>,
                    {
                        let __field0 =
                            match match _serde::de::SeqAccess::next_element::<String>(&mut __seq) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            } {
                                _serde::__private::Some(__value) => __value,
                                _serde::__private::None => {
                                    return _serde::__private::Err(
                                        _serde::de::Error::invalid_length(
                                            0usize,
                                            &"struct User with 4 elements",
                                        ),
                                    );
                                }
                            };
                        let __field1 =
                            match match _serde::de::SeqAccess::next_element::<String>(&mut __seq) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            } {
                                _serde::__private::Some(__value) => __value,
                                _serde::__private::None => {
                                    return _serde::__private::Err(
                                        _serde::de::Error::invalid_length(
                                            1usize,
                                            &"struct User with 4 elements",
                                        ),
                                    );
                                }
                            };
                        let __field2 =
                            match match _serde::de::SeqAccess::next_element::<Platform>(&mut __seq)
                            {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            } {
                                _serde::__private::Some(__value) => __value,
                                _serde::__private::None => {
                                    return _serde::__private::Err(
                                        _serde::de::Error::invalid_length(
                                            2usize,
                                            &"struct User with 4 elements",
                                        ),
                                    );
                                }
                            };
                        let __field3 = match match _serde::de::SeqAccess::next_element::<Permissions>(
                            &mut __seq,
                        ) {
                            _serde::__private::Ok(__val) => __val,
                            _serde::__private::Err(__err) => {
                                return _serde::__private::Err(__err);
                            }
                        } {
                            _serde::__private::Some(__value) => __value,
                            _serde::__private::None => {
                                return _serde::__private::Err(_serde::de::Error::invalid_length(
                                    3usize,
                                    &"struct User with 4 elements",
                                ));
                            }
                        };
                        _serde::__private::Ok(User {
                            name: __field0,
                            id: __field1,
                            platform: __field2,
                            perms: __field3,
                        })
                    }
                    #[inline]
                    fn visit_map<__A>(
                        self,
                        mut __map: __A,
                    ) -> _serde::__private::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::MapAccess<'de>,
                    {
                        let mut __field0: _serde::__private::Option<String> =
                            _serde::__private::None;
                        let mut __field1: _serde::__private::Option<String> =
                            _serde::__private::None;
                        let mut __field2: _serde::__private::Option<Platform> =
                            _serde::__private::None;
                        let mut __field3: _serde::__private::Option<Permissions> =
                            _serde::__private::None;
                        while let _serde::__private::Some(__key) =
                            match _serde::de::MapAccess::next_key::<__Field>(&mut __map) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            }
                        {
                            match __key {
                                __Field::__field0 => {
                                    if _serde::__private::Option::is_some(&__field0) {
                                        return _serde::__private::Err(
                                            <__A::Error as _serde::de::Error>::duplicate_field(
                                                "name",
                                            ),
                                        );
                                    }
                                    __field0 = _serde::__private::Some(
                                        match _serde::de::MapAccess::next_value::<String>(
                                            &mut __map,
                                        ) {
                                            _serde::__private::Ok(__val) => __val,
                                            _serde::__private::Err(__err) => {
                                                return _serde::__private::Err(__err);
                                            }
                                        },
                                    );
                                }
                                __Field::__field1 => {
                                    if _serde::__private::Option::is_some(&__field1) {
                                        return _serde::__private::Err(
                                            <__A::Error as _serde::de::Error>::duplicate_field(
                                                "id",
                                            ),
                                        );
                                    }
                                    __field1 = _serde::__private::Some(
                                        match _serde::de::MapAccess::next_value::<String>(
                                            &mut __map,
                                        ) {
                                            _serde::__private::Ok(__val) => __val,
                                            _serde::__private::Err(__err) => {
                                                return _serde::__private::Err(__err);
                                            }
                                        },
                                    );
                                }
                                __Field::__field2 => {
                                    if _serde::__private::Option::is_some(&__field2) {
                                        return _serde::__private::Err(
                                            <__A::Error as _serde::de::Error>::duplicate_field(
                                                "platform",
                                            ),
                                        );
                                    }
                                    __field2 = _serde::__private::Some(
                                        match _serde::de::MapAccess::next_value::<Platform>(
                                            &mut __map,
                                        ) {
                                            _serde::__private::Ok(__val) => __val,
                                            _serde::__private::Err(__err) => {
                                                return _serde::__private::Err(__err);
                                            }
                                        },
                                    );
                                }
                                __Field::__field3 => {
                                    if _serde::__private::Option::is_some(&__field3) {
                                        return _serde::__private::Err(
                                            <__A::Error as _serde::de::Error>::duplicate_field(
                                                "perms",
                                            ),
                                        );
                                    }
                                    __field3 = _serde::__private::Some(
                                        match _serde::de::MapAccess::next_value::<Permissions>(
                                            &mut __map,
                                        ) {
                                            _serde::__private::Ok(__val) => __val,
                                            _serde::__private::Err(__err) => {
                                                return _serde::__private::Err(__err);
                                            }
                                        },
                                    );
                                }
                                _ => {
                                    let _ = match _serde::de::MapAccess::next_value::<
                                        _serde::de::IgnoredAny,
                                    >(&mut __map)
                                    {
                                        _serde::__private::Ok(__val) => __val,
                                        _serde::__private::Err(__err) => {
                                            return _serde::__private::Err(__err);
                                        }
                                    };
                                }
                            }
                        }
                        let __field0 = match __field0 {
                            _serde::__private::Some(__field0) => __field0,
                            _serde::__private::None => {
                                match _serde::__private::de::missing_field("name") {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                }
                            }
                        };
                        let __field1 = match __field1 {
                            _serde::__private::Some(__field1) => __field1,
                            _serde::__private::None => {
                                match _serde::__private::de::missing_field("id") {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                }
                            }
                        };
                        let __field2 = match __field2 {
                            _serde::__private::Some(__field2) => __field2,
                            _serde::__private::None => {
                                match _serde::__private::de::missing_field("platform") {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                }
                            }
                        };
                        let __field3 = match __field3 {
                            _serde::__private::Some(__field3) => __field3,
                            _serde::__private::None => {
                                match _serde::__private::de::missing_field("perms") {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                }
                            }
                        };
                        _serde::__private::Ok(User {
                            name: __field0,
                            id: __field1,
                            platform: __field2,
                            perms: __field3,
                        })
                    }
                }
                const FIELDS: &'static [&'static str] = &["name", "id", "platform", "perms"];
                _serde::Deserializer::deserialize_struct(
                    __deserializer,
                    "User",
                    FIELDS,
                    __Visitor {
                        marker: _serde::__private::PhantomData::<User>,
                        lifetime: _serde::__private::PhantomData,
                    },
                )
            }
        }
    };
}
mod resp {
    use crate::msg::Platform;
    use serde::Deserialize;
    pub const CHANNEL_NAME: &str = "aussiegg";
    pub const UPSTREAM_CHAN: &str = "aussieup";
    pub const DOWNSTREAM_CHAN: &str = "aussiedown";
    pub struct Response<'a> {
        pub channel: &'a str,
        pub dest: (Platform, &'a str, &'a str),
        pub payload: Payload<'a>,
    }
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl<'de: 'a, 'a> _serde::Deserialize<'de> for Response<'a> {
            fn deserialize<__D>(__deserializer: __D) -> _serde::__private::Result<Self, __D::Error>
            where
                __D: _serde::Deserializer<'de>,
            {
                #[allow(non_camel_case_types)]
                enum __Field {
                    __field0,
                    __field1,
                    __field2,
                    __ignore,
                }
                struct __FieldVisitor;
                impl<'de> _serde::de::Visitor<'de> for __FieldVisitor {
                    type Value = __Field;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(__formatter, "field identifier")
                    }
                    fn visit_u64<__E>(
                        self,
                        __value: u64,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            0u64 => _serde::__private::Ok(__Field::__field0),
                            1u64 => _serde::__private::Ok(__Field::__field1),
                            2u64 => _serde::__private::Ok(__Field::__field2),
                            _ => _serde::__private::Ok(__Field::__ignore),
                        }
                    }
                    fn visit_str<__E>(
                        self,
                        __value: &str,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            "channel" => _serde::__private::Ok(__Field::__field0),
                            "dest" => _serde::__private::Ok(__Field::__field1),
                            "payload" => _serde::__private::Ok(__Field::__field2),
                            _ => _serde::__private::Ok(__Field::__ignore),
                        }
                    }
                    fn visit_bytes<__E>(
                        self,
                        __value: &[u8],
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            b"channel" => _serde::__private::Ok(__Field::__field0),
                            b"dest" => _serde::__private::Ok(__Field::__field1),
                            b"payload" => _serde::__private::Ok(__Field::__field2),
                            _ => _serde::__private::Ok(__Field::__ignore),
                        }
                    }
                }
                impl<'de> _serde::Deserialize<'de> for __Field {
                    #[inline]
                    fn deserialize<__D>(
                        __deserializer: __D,
                    ) -> _serde::__private::Result<Self, __D::Error>
                    where
                        __D: _serde::Deserializer<'de>,
                    {
                        _serde::Deserializer::deserialize_identifier(__deserializer, __FieldVisitor)
                    }
                }
                struct __Visitor<'de: 'a, 'a> {
                    marker: _serde::__private::PhantomData<Response<'a>>,
                    lifetime: _serde::__private::PhantomData<&'de ()>,
                }
                impl<'de: 'a, 'a> _serde::de::Visitor<'de> for __Visitor<'de, 'a> {
                    type Value = Response<'a>;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(__formatter, "struct Response")
                    }
                    #[inline]
                    fn visit_seq<__A>(
                        self,
                        mut __seq: __A,
                    ) -> _serde::__private::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::SeqAccess<'de>,
                    {
                        let __field0 = match match _serde::de::SeqAccess::next_element::<&'a str>(
                            &mut __seq,
                        ) {
                            _serde::__private::Ok(__val) => __val,
                            _serde::__private::Err(__err) => {
                                return _serde::__private::Err(__err);
                            }
                        } {
                            _serde::__private::Some(__value) => __value,
                            _serde::__private::None => {
                                return _serde::__private::Err(_serde::de::Error::invalid_length(
                                    0usize,
                                    &"struct Response with 3 elements",
                                ));
                            }
                        };
                        let __field1 = match match _serde::de::SeqAccess::next_element::<(
                            Platform,
                            &'a str,
                            &'a str,
                        )>(&mut __seq)
                        {
                            _serde::__private::Ok(__val) => __val,
                            _serde::__private::Err(__err) => {
                                return _serde::__private::Err(__err);
                            }
                        } {
                            _serde::__private::Some(__value) => __value,
                            _serde::__private::None => {
                                return _serde::__private::Err(_serde::de::Error::invalid_length(
                                    1usize,
                                    &"struct Response with 3 elements",
                                ));
                            }
                        };
                        let __field2 = match match _serde::de::SeqAccess::next_element::<Payload<'a>>(
                            &mut __seq,
                        ) {
                            _serde::__private::Ok(__val) => __val,
                            _serde::__private::Err(__err) => {
                                return _serde::__private::Err(__err);
                            }
                        } {
                            _serde::__private::Some(__value) => __value,
                            _serde::__private::None => {
                                return _serde::__private::Err(_serde::de::Error::invalid_length(
                                    2usize,
                                    &"struct Response with 3 elements",
                                ));
                            }
                        };
                        _serde::__private::Ok(Response {
                            channel: __field0,
                            dest: __field1,
                            payload: __field2,
                        })
                    }
                    #[inline]
                    fn visit_map<__A>(
                        self,
                        mut __map: __A,
                    ) -> _serde::__private::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::MapAccess<'de>,
                    {
                        let mut __field0: _serde::__private::Option<&'a str> =
                            _serde::__private::None;
                        let mut __field1: _serde::__private::Option<(Platform, &'a str, &'a str)> =
                            _serde::__private::None;
                        let mut __field2: _serde::__private::Option<Payload<'a>> =
                            _serde::__private::None;
                        while let _serde::__private::Some(__key) =
                            match _serde::de::MapAccess::next_key::<__Field>(&mut __map) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            }
                        {
                            match __key {
                                __Field::__field0 => {
                                    if _serde::__private::Option::is_some(&__field0) {
                                        return _serde::__private::Err(
                                            <__A::Error as _serde::de::Error>::duplicate_field(
                                                "channel",
                                            ),
                                        );
                                    }
                                    __field0 = _serde::__private::Some(
                                        match _serde::de::MapAccess::next_value::<&'a str>(
                                            &mut __map,
                                        ) {
                                            _serde::__private::Ok(__val) => __val,
                                            _serde::__private::Err(__err) => {
                                                return _serde::__private::Err(__err);
                                            }
                                        },
                                    );
                                }
                                __Field::__field1 => {
                                    if _serde::__private::Option::is_some(&__field1) {
                                        return _serde::__private::Err(
                                            <__A::Error as _serde::de::Error>::duplicate_field(
                                                "dest",
                                            ),
                                        );
                                    }
                                    __field1 = _serde::__private::Some(
                                        match _serde::de::MapAccess::next_value::<(
                                            Platform,
                                            &'a str,
                                            &'a str,
                                        )>(&mut __map)
                                        {
                                            _serde::__private::Ok(__val) => __val,
                                            _serde::__private::Err(__err) => {
                                                return _serde::__private::Err(__err);
                                            }
                                        },
                                    );
                                }
                                __Field::__field2 => {
                                    if _serde::__private::Option::is_some(&__field2) {
                                        return _serde::__private::Err(
                                            <__A::Error as _serde::de::Error>::duplicate_field(
                                                "payload",
                                            ),
                                        );
                                    }
                                    __field2 = _serde::__private::Some(
                                        match _serde::de::MapAccess::next_value::<Payload<'a>>(
                                            &mut __map,
                                        ) {
                                            _serde::__private::Ok(__val) => __val,
                                            _serde::__private::Err(__err) => {
                                                return _serde::__private::Err(__err);
                                            }
                                        },
                                    );
                                }
                                _ => {
                                    let _ = match _serde::de::MapAccess::next_value::<
                                        _serde::de::IgnoredAny,
                                    >(&mut __map)
                                    {
                                        _serde::__private::Ok(__val) => __val,
                                        _serde::__private::Err(__err) => {
                                            return _serde::__private::Err(__err);
                                        }
                                    };
                                }
                            }
                        }
                        let __field0 = match __field0 {
                            _serde::__private::Some(__field0) => __field0,
                            _serde::__private::None => {
                                match _serde::__private::de::missing_field("channel") {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                }
                            }
                        };
                        let __field1 = match __field1 {
                            _serde::__private::Some(__field1) => __field1,
                            _serde::__private::None => {
                                match _serde::__private::de::missing_field("dest") {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                }
                            }
                        };
                        let __field2 = match __field2 {
                            _serde::__private::Some(__field2) => __field2,
                            _serde::__private::None => {
                                match _serde::__private::de::missing_field("payload") {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                }
                            }
                        };
                        _serde::__private::Ok(Response {
                            channel: __field0,
                            dest: __field1,
                            payload: __field2,
                        })
                    }
                }
                const FIELDS: &'static [&'static str] = &["channel", "dest", "payload"];
                _serde::Deserializer::deserialize_struct(
                    __deserializer,
                    "Response",
                    FIELDS,
                    __Visitor {
                        marker: _serde::__private::PhantomData::<Response<'a>>,
                        lifetime: _serde::__private::PhantomData,
                    },
                )
            }
        }
    };
    pub enum Payload<'a> {
        #[serde(rename(deserialize = "ping"))]
        PingRequest(crate::msg::User, &'a str, Option<&'a str>),
    }
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl<'de: 'a, 'a> _serde::Deserialize<'de> for Payload<'a> {
            fn deserialize<__D>(__deserializer: __D) -> _serde::__private::Result<Self, __D::Error>
            where
                __D: _serde::Deserializer<'de>,
            {
                #[allow(non_camel_case_types)]
                enum __Field {
                    __field0,
                }
                struct __FieldVisitor;
                impl<'de> _serde::de::Visitor<'de> for __FieldVisitor {
                    type Value = __Field;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(__formatter, "variant identifier")
                    }
                    fn visit_u64<__E>(
                        self,
                        __value: u64,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            0u64 => _serde::__private::Ok(__Field::__field0),
                            _ => _serde::__private::Err(_serde::de::Error::invalid_value(
                                _serde::de::Unexpected::Unsigned(__value),
                                &"variant index 0 <= i < 1",
                            )),
                        }
                    }
                    fn visit_str<__E>(
                        self,
                        __value: &str,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            "ping" => _serde::__private::Ok(__Field::__field0),
                            _ => _serde::__private::Err(_serde::de::Error::unknown_variant(
                                __value, VARIANTS,
                            )),
                        }
                    }
                    fn visit_bytes<__E>(
                        self,
                        __value: &[u8],
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            b"ping" => _serde::__private::Ok(__Field::__field0),
                            _ => {
                                let __value = &_serde::__private::from_utf8_lossy(__value);
                                _serde::__private::Err(_serde::de::Error::unknown_variant(
                                    __value, VARIANTS,
                                ))
                            }
                        }
                    }
                }
                impl<'de> _serde::Deserialize<'de> for __Field {
                    #[inline]
                    fn deserialize<__D>(
                        __deserializer: __D,
                    ) -> _serde::__private::Result<Self, __D::Error>
                    where
                        __D: _serde::Deserializer<'de>,
                    {
                        _serde::Deserializer::deserialize_identifier(__deserializer, __FieldVisitor)
                    }
                }
                struct __Visitor<'de: 'a, 'a> {
                    marker: _serde::__private::PhantomData<Payload<'a>>,
                    lifetime: _serde::__private::PhantomData<&'de ()>,
                }
                impl<'de: 'a, 'a> _serde::de::Visitor<'de> for __Visitor<'de, 'a> {
                    type Value = Payload<'a>;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(__formatter, "enum Payload")
                    }
                    fn visit_enum<__A>(
                        self,
                        __data: __A,
                    ) -> _serde::__private::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::EnumAccess<'de>,
                    {
                        match match _serde::de::EnumAccess::variant(__data) {
                            _serde::__private::Ok(__val) => __val,
                            _serde::__private::Err(__err) => {
                                return _serde::__private::Err(__err);
                            }
                        } {
                            (__Field::__field0, __variant) => {
                                struct __Visitor<'de: 'a, 'a> {
                                    marker: _serde::__private::PhantomData<Payload<'a>>,
                                    lifetime: _serde::__private::PhantomData<&'de ()>,
                                }
                                impl<'de: 'a, 'a> _serde::de::Visitor<'de> for __Visitor<'de, 'a> {
                                    type Value = Payload<'a>;
                                    fn expecting(
                                        &self,
                                        __formatter: &mut _serde::__private::Formatter,
                                    ) -> _serde::__private::fmt::Result
                                    {
                                        _serde::__private::Formatter::write_str(
                                            __formatter,
                                            "tuple variant Payload::PingRequest",
                                        )
                                    }
                                    #[inline]
                                    fn visit_seq<__A>(
                                        self,
                                        mut __seq: __A,
                                    ) -> _serde::__private::Result<Self::Value, __A::Error>
                                    where
                                        __A: _serde::de::SeqAccess<'de>,
                                    {
                                        let __field0 =
                                            match match _serde::de::SeqAccess::next_element::<
                                                crate::msg::User,
                                            >(
                                                &mut __seq
                                            ) {
                                                _serde::__private::Ok(__val) => __val,
                                                _serde::__private::Err(__err) => {
                                                    return _serde::__private::Err(__err);
                                                }
                                            } {
                                                _serde::__private::Some(__value) => __value,
                                                _serde::__private::None => {
                                                    return _serde :: __private :: Err (_serde :: de :: Error :: invalid_length (0usize , & "tuple variant Payload::PingRequest with 3 elements")) ;
                                                }
                                            };
                                        let __field1 =
                                            match match _serde::de::SeqAccess::next_element::<&'a str>(
                                                &mut __seq,
                                            ) {
                                                _serde::__private::Ok(__val) => __val,
                                                _serde::__private::Err(__err) => {
                                                    return _serde::__private::Err(__err);
                                                }
                                            } {
                                                _serde::__private::Some(__value) => __value,
                                                _serde::__private::None => {
                                                    return _serde :: __private :: Err (_serde :: de :: Error :: invalid_length (1usize , & "tuple variant Payload::PingRequest with 3 elements")) ;
                                                }
                                            };
                                        let __field2 =
                                            match match _serde::de::SeqAccess::next_element::<
                                                Option<&'a str>,
                                            >(
                                                &mut __seq
                                            ) {
                                                _serde::__private::Ok(__val) => __val,
                                                _serde::__private::Err(__err) => {
                                                    return _serde::__private::Err(__err);
                                                }
                                            } {
                                                _serde::__private::Some(__value) => __value,
                                                _serde::__private::None => {
                                                    return _serde :: __private :: Err (_serde :: de :: Error :: invalid_length (2usize , & "tuple variant Payload::PingRequest with 3 elements")) ;
                                                }
                                            };
                                        _serde::__private::Ok(Payload::PingRequest(
                                            __field0, __field1, __field2,
                                        ))
                                    }
                                }
                                _serde::de::VariantAccess::tuple_variant(
                                    __variant,
                                    3usize,
                                    __Visitor {
                                        marker: _serde::__private::PhantomData::<Payload<'a>>,
                                        lifetime: _serde::__private::PhantomData,
                                    },
                                )
                            }
                        }
                    }
                }
                const VARIANTS: &'static [&'static str] = &["ping"];
                _serde::Deserializer::deserialize_enum(
                    __deserializer,
                    "Payload",
                    VARIANTS,
                    __Visitor {
                        marker: _serde::__private::PhantomData::<Payload<'a>>,
                        lifetime: _serde::__private::PhantomData,
                    },
                )
            }
        }
    };
}
use bb8_redis::bb8::Pool;
use bb8_redis::RedisConnectionManager;
use chrono::prelude::*;
use parking_lot::Mutex;
use redis::{AsyncCommands, RedisError};
use resp::{Payload, Response};
use serde_json::json;
use serenity::futures::StreamExt;
use serenity::model::channel::Message;
use serenity::model::gateway::{ActivityType, GatewayIntents, Presence, Ready};
use serenity::model::id::UserId;
use serenity::prelude::*;
use serenity::{async_trait, CacheAndHttp};
use std::sync::Arc;
use tokio::task::JoinHandle;
pub type RedisPool = Pool<RedisConnectionManager>;
struct Handler {
    redis: RedisPool,
    was_streaming: Arc<Mutex<bool>>,
    prev_url: Arc<Mutex<String>>,
    cancel_task: Arc<Mutex<Option<JoinHandle<()>>>>,
}
const AUSSIEGG_ID: u64 = 187926644311326720;
const STATUS_DEBOUNCE_SECS: u64 = 8;
impl EventHandler for Handler {
    #[allow(
        clippy::let_unit_value,
        clippy::no_effect_underscore_binding,
        clippy::shadow_same,
        clippy::type_complexity,
        clippy::type_repetition_in_bounds,
        clippy::used_underscore_binding
    )]
    fn message<'life0, 'async_trait>(
        &'life0 self,
        _ctx: Context,
        msg: Message,
    ) -> ::core::pin::Pin<
        Box<dyn ::core::future::Future<Output = ()> + ::core::marker::Send + 'async_trait>,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            let __self = self;
            let _ctx = _ctx;
            let msg = msg;
            let _: () = {
                ::std::io::_print(::core::fmt::Arguments::new_v1(
                    &["Received message: ", " from ", " ", "\n"],
                    &[
                        ::core::fmt::ArgumentV1::new_display(&msg.content),
                        ::core::fmt::ArgumentV1::new_debug(&msg.author.id),
                        ::core::fmt::ArgumentV1::new_debug(&msg.author.name),
                    ],
                ));
            };
        })
    }
    #[allow(
        clippy::let_unit_value,
        clippy::no_effect_underscore_binding,
        clippy::shadow_same,
        clippy::type_complexity,
        clippy::type_repetition_in_bounds,
        clippy::used_underscore_binding
    )]
    fn presence_update<'life0, 'async_trait>(
        &'life0 self,
        _ctx: Context,
        new_data: Presence,
    ) -> ::core::pin::Pin<
        Box<dyn ::core::future::Future<Output = ()> + ::core::marker::Send + 'async_trait>,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            let __self = self;
            let _ctx = _ctx;
            let new_data = new_data;
            let _: () = {
                if new_data.user.id != AUSSIEGG_ID {
                    return;
                }
                let local: DateTime<Local> = Local::now();
                ::std::io::_print(::core::fmt::Arguments::new_v1(
                    &[
                        "-------------------presence update-------------------\n",
                        "\n",
                    ],
                    &[::core::fmt::ArgumentV1::new_display(&local)],
                ));
                let is_streaming = new_data
                    .activities
                    .iter()
                    .find(|activity| (activity.kind == ActivityType::Streaming));
                let (is_streaming, stream_url) = if let Some(act) = is_streaming {
                    (true, act.url.as_ref())
                } else {
                    (false, None)
                };
                let was_streaming = *__self.was_streaming.lock();
                ::std::io::_print(::core::fmt::Arguments::new_v1(
                    &["was streaming: ", ", is streaming: ", "\n"],
                    &[
                        ::core::fmt::ArgumentV1::new_display(&was_streaming),
                        ::core::fmt::ArgumentV1::new_display(&is_streaming),
                    ],
                ));
                ::std::io::_print(::core::fmt::Arguments::new_v1(
                    &["url: ", "\n"],
                    &[::core::fmt::ArgumentV1::new_debug(&stream_url)],
                ));
                if !was_streaming && is_streaming {
                    if let Some(ref h) = *__self.cancel_task.lock() {
                        h.abort();
                        ::std::io::_print(::core::fmt::Arguments::new_v1(
                            &["Aborted cancel task\n"],
                            &[],
                        ));
                    }
                    let new_url = stream_url.unwrap().to_string();
                    let resp = ::serde_json::Value::Array(<[_]>::into_vec(box [
                        ::serde_json::to_value(&resp::CHANNEL_NAME).unwrap(),
                        ::serde_json::to_value(&2_u8).unwrap(),
                        ::serde_json::to_value(&4_u8).unwrap(),
                        ::serde_json::to_value(&0_u8).unwrap(),
                        ::serde_json::to_value(&&new_url).unwrap(),
                    ]))
                    .to_string();
                    {
                        let mut was_streaming = __self.was_streaming.lock();
                        *was_streaming = is_streaming;
                        let mut prev_url = __self.prev_url.lock();
                        *prev_url = new_url;
                    }
                    __self
                        .redis
                        .get()
                        .await
                        .unwrap()
                        .publish::<&str, String, ()>(resp::UPSTREAM_CHAN, resp)
                        .await
                        .unwrap();
                } else if was_streaming {
                    if let Some(ref h) = *__self.cancel_task.lock() {
                        h.abort();
                        ::std::io::_print(::core::fmt::Arguments::new_v1(
                            &["Aborted cancel task\n"],
                            &[],
                        ));
                    }
                    if is_streaming {
                        return;
                    }
                    let was_streaming = __self.was_streaming.clone();
                    let prev_url = __self.prev_url.clone();
                    let redis = __self.redis.clone();
                    let cancel_task = __self.cancel_task.clone();
                    ::std::io::_print(::core::fmt::Arguments::new_v1(
                        &["Spawning cancel task\n"],
                        &[],
                    ));
                    let h = tokio::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_secs(STATUS_DEBOUNCE_SECS))
                            .await;
                        ::std::io::_print(::core::fmt::Arguments::new_v1(
                            &["IN CANCEL TASK\n"],
                            &[],
                        ));
                        let resp = {
                            let mut prev_url = prev_url.lock();
                            let resp = ::serde_json::Value::Array(<[_]>::into_vec(box [
                                ::serde_json::to_value(&resp::CHANNEL_NAME).unwrap(),
                                ::serde_json::to_value(&2_u8).unwrap(),
                                ::serde_json::to_value(&4_u8).unwrap(),
                                ::serde_json::to_value(&1_u8).unwrap(),
                                ::serde_json::to_value(&prev_url.as_str()).unwrap(),
                            ]))
                            .to_string();
                            prev_url.clear();
                            resp
                        };
                        {
                            let mut was_streaming = was_streaming.lock();
                            *was_streaming = false;
                            let mut cancel_task = cancel_task.lock();
                            *cancel_task = None;
                        }
                        redis
                            .get()
                            .await
                            .unwrap()
                            .publish::<&str, String, ()>(resp::UPSTREAM_CHAN, resp)
                            .await
                            .unwrap();
                    });
                    let mut cancel_task = __self.cancel_task.lock();
                    *cancel_task = Some(h);
                }
            };
        })
    }
    #[allow(
        clippy::let_unit_value,
        clippy::no_effect_underscore_binding,
        clippy::shadow_same,
        clippy::type_complexity,
        clippy::type_repetition_in_bounds,
        clippy::used_underscore_binding
    )]
    fn ready<'life0, 'async_trait>(
        &'life0 self,
        __arg1: Context,
        ready: Ready,
    ) -> ::core::pin::Pin<
        Box<dyn ::core::future::Future<Output = ()> + ::core::marker::Send + 'async_trait>,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            let __self = self;
            let _ = __arg1;
            let ready = ready;
            let _: () = {
                ::std::io::_print(::core::fmt::Arguments::new_v1(
                    &["", " is connected!\n"],
                    &[::core::fmt::ArgumentV1::new_display(&ready.user.name)],
                ));
            };
        })
    }
}
fn main() {
    let body = async {
        dotenv::dotenv().unwrap();
        let amt = "$8.88";
        let name = "SDJGKNASFGOJB2@@\nWRFGRASDFadf";
        let regex_amt = regex::Regex::new(r"\{amount\}").unwrap();
        let regex_name = regex::Regex::new(r"\{name\}").unwrap();
        let redis_pool = init_redis().await.unwrap();
        let redis_client =
            redis::Client::open(dotenv::var("REDIS_URL").expect("REDIS_URL env var")).unwrap();
        let handler = Handler {
            redis: redis_pool.clone(),
            was_streaming: Arc::new(Mutex::new(false)),
            prev_url: Arc::new(Mutex::new("".into())),
            cancel_task: Arc::new(Mutex::new(None)),
        };
        let token = dotenv::var("DISCORD_TOKEN").expect("Expected a token in the environment");
        let intents = GatewayIntents::GUILDS
            | GatewayIntents::GUILD_MESSAGES
            | GatewayIntents::MESSAGE_CONTENT
            | GatewayIntents::DIRECT_MESSAGES
            | GatewayIntents::GUILD_PRESENCES;
        let mut client = Client::builder(token, intents)
            .event_handler(handler)
            .await
            .expect("Error creating client");
        let cache = client.cache_and_http.clone();
        tokio::spawn(async move {
            ::std::io::_print(::core::fmt::Arguments::new_v1(
                &["\u{1b}[92m------------------Starting chat loop------------------\u{1b}[0m\n"],
                &[],
            ));
            loop {
                start_redis_loop(&redis_client, cache.clone()).await;
                :: std :: io :: _print (:: core :: fmt :: Arguments :: new_v1 (& ["\u{1b}[91m-----------------Restarting chat loop-----------------\u{1b}[0m\n"] , & [])) ;
            }
        });
        if let Err(why) = client.start().await {
            ::std::io::_print(::core::fmt::Arguments::new_v1(
                &["Client error: ", "\n"],
                &[::core::fmt::ArgumentV1::new_debug(&why)],
            ));
        }
    };
    #[allow(clippy::expect_used)]
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed building the Runtime")
        .block_on(body)
}
async fn init_redis() -> Result<RedisPool, RedisError> {
    let manager = bb8_redis::RedisConnectionManager::new(
        dotenv::var("REDIS_URL").expect("REDIS_URL env var"),
    )?;
    Pool::builder().max_size(10).build(manager).await
}
/// Process chat messages
async fn start_redis_loop(client: &redis::Client, cache: Arc<CacheAndHttp>) -> Option<()> {
    let mut sub = client.get_tokio_connection().await.ok()?.into_pubsub();
    sub.subscribe(resp::DOWNSTREAM_CHAN).await.unwrap();
    let mut sub = sub.into_on_message();
    loop {
        let msg = sub.next().await?.get_payload::<String>().ok()?;
        ::std::io::_print(::core::fmt::Arguments::new_v1(
            &["redis recv: ", "\n"],
            &[::core::fmt::ArgumentV1::new_display(&msg)],
        ));
        let data = match serde_json::from_str::<Response>(&msg).ok() {
            Some(data) => data,
            _ => continue,
        };
        if data.channel != resp::CHANNEL_NAME {
            continue;
        }
        if let Payload::PingRequest(user, pingee_id, msg) = data.payload {
            let msg = if let Some(msg) = msg {
                {
                    let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                        &["", " from ", " pinged you with a message: "],
                        &[
                            ::core::fmt::ArgumentV1::new_display(&user.name),
                            ::core::fmt::ArgumentV1::new_debug(&user.platform),
                            ::core::fmt::ArgumentV1::new_debug(&msg),
                        ],
                    ));
                    res
                }
            } else {
                {
                    let res = ::alloc::fmt::format(::core::fmt::Arguments::new_v1(
                        &["", " from ", " pinged you!"],
                        &[
                            ::core::fmt::ArgumentV1::new_display(&user.name),
                            ::core::fmt::ArgumentV1::new_debug(&user.platform),
                        ],
                    ));
                    res
                }
            };
            let id = pingee_id.parse::<u64>().ok()?;
            let pingee = UserId(id).to_user(&cache).await.unwrap();
            pingee
                .direct_message(&cache, |m| m.content(msg))
                .await
                .unwrap();
        }
    }
}
