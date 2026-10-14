// Spine Runtimes License Agreement
// Last updated April 5, 2025. Replaces all prior versions.
//
// Copyright (c) 2013-2025, Esoteric Software LLC
//
// Integration of the Spine Runtimes into software or otherwise creating
// derivative works of the Spine Runtimes is permitted under the terms and
// conditions of Section 2 of the Spine Editor License Agreement:
// http://esotericsoftware.com/spine-editor-license
//
// Otherwise, it is permitted to integrate the Spine Runtimes into software
// or otherwise create derivative works of the Spine Runtimes (collectively,
// "Products"), provided that each user of the Products must obtain their own
// Spine Editor license and redistribution of the Products in any form must
// include this license and copyright notice.
//
// THE SPINE RUNTIMES ARE PROVIDED BY ESOTERIC SOFTWARE LLC "AS IS" AND ANY
// EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
// WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL ESOTERIC SOFTWARE LLC BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
// (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES,
// BUSINESS INTERRUPTION, OR LOSS OF USE, DATA, OR PROFITS) HOWEVER CAUSED AND
// ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF
// THE SPINE RUNTIMES, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//
// Unofficial Rust port of the Spine 4.3 runtime (spine-cpp). Not an official Spine runtime.

//! JSON access with the reference runtime's semantics, on top of `serde_json`.
//!
//! Parsing is `serde_json`'s (with `preserve_order`, since animations, skins and attachments are
//! order-sensitive). Numbers are the one exception: `arbitrary_precision` keeps each
//! number's original text, and [`JsonExt`] re-reads it with the reference algorithm (accumulated
//! in `f64`, then truncated to `f32`/`i32`), so loaded values are bit-identical to the C++
//! runtime's rather than merely correctly rounded.
//!
//! Known difference: `serde_json` keeps the *last* of duplicate object keys where the reference
//! uses the first. Spine exports never contain duplicate keys.

use serde_json::Value;
pub use serde_json::{Error as JsonError, Value as Json};

/// Parses a JSON document.
pub fn parse(text: &str) -> Result<Json, JsonError> {
    serde_json::from_str(text)
}

/// The reference runtime's number parse: `(value as f32, value truncated to i32)`.
///
/// Not correctly rounded: digits are accumulated in `f64` and the fraction and exponent are
/// applied with `pow`, exactly like the C++ `Json::parseNumber`.
pub fn reference_number(text: &str) -> (f32, i32) {
    let s = text.as_bytes();
    let mut pos = 0;
    let peek = |pos: usize| s.get(pos).copied().unwrap_or(0);
    let mut result = 0.0f64;
    let mut negative = false;
    if peek(pos) == b'-' {
        negative = true;
        pos += 1;
    }
    while peek(pos).is_ascii_digit() {
        result = result * 10.0 + (peek(pos) - b'0') as f64;
        pos += 1;
    }
    if peek(pos) == b'.' {
        let mut fraction = 0.0f64;
        let mut n = 0i32;
        pos += 1;
        while peek(pos).is_ascii_digit() {
            fraction = fraction * 10.0 + (peek(pos) - b'0') as f64;
            pos += 1;
            n += 1;
        }
        result += fraction / 10f64.powf(n as f64);
    }
    if negative {
        result = -result;
    }
    if matches!(peek(pos), b'e' | b'E') {
        let mut exponent = 0.0f64;
        let mut exp_negative = false;
        pos += 1;
        if peek(pos) == b'-' {
            exp_negative = true;
            pos += 1;
        } else if peek(pos) == b'+' {
            pos += 1;
        }
        while peek(pos).is_ascii_digit() {
            exponent = exponent * 10.0 + (peek(pos) - b'0') as f64;
            pos += 1;
        }
        if exp_negative {
            result /= 10f64.powf(exponent);
        } else {
            result *= 10f64.powf(exponent);
        }
    }
    (result as f32, result as i32)
}

/// Accessors with the reference runtime's lookup and defaulting rules.
pub trait JsonExt {
    /// The first value for `name` in an object. Case-insensitive, like the reference.
    fn get_ci(&self, name: &str) -> Option<&Json>;
    /// Number of children of an array or object, 0 otherwise.
    fn child_count(&self) -> usize;
    /// Children as `(name, value)`; names are empty for arrays.
    fn children(&self) -> Box<dyn Iterator<Item = (&str, &Json)> + '_>;
    /// The string value of `name`; `None` if missing or not a string.
    fn str(&self, name: &str) -> Option<&str>;
    /// The numeric value of `name`; `default` if missing, 0 if present but not a number.
    fn float(&self, name: &str, default: f32) -> f32;
    /// Like `float` but truncated to `i32`.
    fn int(&self, name: &str, default: i32) -> i32;
    fn boolean(&self, name: &str, default: bool) -> bool;
    /// This value as a number: `f32` of a number, 0 otherwise.
    fn as_float(&self) -> f32;
    fn as_int(&self) -> i32;
    /// An array's elements as floats, or `None` if this is not a container.
    fn float_array(&self) -> Option<Vec<f32>>;
    fn u16_array(&self) -> Option<Vec<u16>>;
}

impl JsonExt for Value {
    fn get_ci(&self, name: &str) -> Option<&Json> {
        match self {
            Value::Object(map) => map
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v),
            _ => None,
        }
    }

    fn child_count(&self) -> usize {
        match self {
            Value::Array(v) => v.len(),
            Value::Object(m) => m.len(),
            _ => 0,
        }
    }

    fn children(&self) -> Box<dyn Iterator<Item = (&str, &Json)> + '_> {
        match self {
            Value::Array(v) => Box::new(v.iter().map(|j| ("", j))),
            Value::Object(m) => Box::new(m.iter().map(|(k, v)| (k.as_str(), v))),
            _ => Box::new(std::iter::empty()),
        }
    }

    fn str(&self, name: &str) -> Option<&str> {
        self.get_ci(name)?.as_str()
    }

    fn float(&self, name: &str, default: f32) -> f32 {
        match self.get_ci(name) {
            None => default,
            Some(v) => v.as_float(),
        }
    }

    fn int(&self, name: &str, default: i32) -> i32 {
        match self.get_ci(name) {
            None => default,
            Some(v) => v.as_int(),
        }
    }

    fn boolean(&self, name: &str, default: bool) -> bool {
        match self.get_ci(name) {
            None => default,
            Some(Value::String(s)) => s == "true",
            Some(Value::Null) => false,
            Some(Value::Number(n)) => reference_number(&n.to_string()).0 != 0.0,
            Some(Value::Bool(b)) => *b,
            Some(_) => default,
        }
    }

    fn as_float(&self) -> f32 {
        match self {
            Value::Number(n) => reference_number(&n.to_string()).0,
            _ => 0.0,
        }
    }

    fn as_int(&self) -> i32 {
        match self {
            Value::Number(n) => reference_number(&n.to_string()).1,
            Value::Bool(true) => 1,
            _ => 0,
        }
    }

    fn float_array(&self) -> Option<Vec<f32>> {
        match self {
            Value::Array(_) | Value::Object(_) => {
                Some(self.children().map(|(_, j)| j.as_float()).collect())
            }
            _ => None,
        }
    }

    fn u16_array(&self) -> Option<Vec<u16>> {
        match self {
            Value::Array(_) | Value::Object(_) => {
                Some(self.children().map(|(_, j)| j.as_int() as u16).collect())
            }
            _ => None,
        }
    }
}
