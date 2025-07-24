use serde::{Deserialize, Serialize};

/// Core User entity - represents the business domain
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    pub id: Option<i32>, // None for new users before persistence
    pub username: String,
    pub email: String,
}

impl User {
    pub fn new(username: String, email: String) -> Self {
        Self {
            id: None,
            username,
            email,
        }
    }

    pub fn with_id(id: i32, username: String, email: String) -> Self {
        Self {
            id: Some(id),
            username,
            email,
        }
    }

    pub fn validate(&self) -> Result<(), crate::DomainError> {
        if self.username.trim().is_empty() {
            return Err(crate::DomainError::ValidationError("Username cannot be empty".to_string()));
        }
        
        if self.email.trim().is_empty() {
            return Err(crate::DomainError::ValidationError("Email cannot be empty".to_string()));
        }
        
        if !self.email.contains('@') {
            return Err(crate::DomainError::ValidationError("Invalid email format".to_string()));
        }
        
        Ok(())
    }
}
