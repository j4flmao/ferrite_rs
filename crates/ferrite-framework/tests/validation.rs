//! End-to-end tests for `#[derive(Validate)]` and `ValidationPipe` via the
//! `ferrite-framework` facade (the derive emits `::ferrite_framework::validation::...` paths).

use ferrite_framework::validation::Validate as ValidateTrait; // trait for method calls
use ferrite_framework::Validate; // derive macro
use ferrite_framework::{Pipe, ValidationErrors, ValidationPipe};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Validate)]
struct SignupDto {
    #[validate(email)]
    email: String,
    #[validate(length(min = 3, max = 32))]
    name: String,
    #[validate(range(min = 13, max = 150))]
    age: u8,
}

fn valid() -> SignupDto {
    SignupDto {
        email: "ana@example.com".into(),
        name: "Ana".into(),
        age: 30,
    }
}

#[test]
fn valid_dto_passes() {
    assert!(valid().validate().is_ok());
}

#[test]
fn invalid_dto_reports_each_field() {
    let dto = SignupDto {
        email: "not-an-email".into(),
        name: "x".into(),
        age: 200,
    };
    let ValidationErrors(errors) = dto.validate().unwrap_err();
    let fields: Vec<&str> = errors.iter().map(|e| e.field.as_str()).collect();
    let messages: Vec<&str> = errors.iter().map(|e| e.message.as_str()).collect();
    assert_eq!(fields, vec!["email", "name", "age"]);
    assert!(messages.iter().any(|m| m.contains("invalid email")));
    assert!(messages.iter().any(|m| m.contains("at least 3")));
    assert!(messages.iter().any(|m| m.contains("at most 150")));
}

#[test]
fn validation_pipe_passes_valid_dto_through() {
    let out = ValidationPipe.transform(valid()).expect("should pass");
    assert_eq!(out.name, "Ana");
}

#[test]
fn validation_pipe_rejects_invalid_dto() {
    let dto = SignupDto {
        email: "x".into(),
        name: "n".into(),
        age: 1,
    };
    let err = ValidationPipe.transform(dto).unwrap_err();
    assert!(
        matches!(err, ferrite_framework::PipeError::Response(_)),
        "expected a 422 pipe response, got {err:?}"
    );
}
