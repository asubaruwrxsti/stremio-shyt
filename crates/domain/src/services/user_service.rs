use crate::entities::User;
use crate::repositories::UserRepository;
use crate::errors::DomainError;
use std::sync::Arc;

/// User Service - Contains business logic
/// This is the APPLICATION LAYER in clean architecture
pub struct UserService {
    user_repository: Arc<dyn UserRepository>,
}

impl UserService {
    pub fn new(user_repository: Arc<dyn UserRepository>) -> Self {
        Self { user_repository }
    }

    /// Create a new user with business validation
    pub async fn create_user(&self, username: String, email: String) -> Result<User, DomainError> {
        let user = User::new(username, email);
        
        // Business validation
        user.validate()?;
        
        // Check if username already exists
        if let Some(_) = self.user_repository.find_by_username(&user.username).await? {
            return Err(DomainError::UsernameAlreadyExists(user.username));
        }
        
        // Check if email already exists
        if let Some(_) = self.user_repository.find_by_email(&user.email).await? {
            return Err(DomainError::EmailAlreadyExists(user.email));
        }
        
        // Save the user
        self.user_repository.save(&user).await
    }

    /// Get user by ID
    pub async fn get_user_by_id(&self, id: i32) -> Result<User, DomainError> {
        match self.user_repository.find_by_id(id).await? {
            Some(user) => Ok(user),
            None => Err(DomainError::UserNotFound(id)),
        }
    }

    /// Update user with business validation
    pub async fn update_user(&self, user: User) -> Result<User, DomainError> {
        // Ensure user has an ID
        let user_id = user.id.ok_or_else(|| {
            DomainError::ValidationError("User ID is required for updates".to_string())
        })?;

        // Validate the user
        user.validate()?;

        // Check if user exists
        self.get_user_by_id(user_id).await?;

        // Check if new username conflicts with another user
        if let Some(existing_user) = self.user_repository.find_by_username(&user.username).await? {
            if existing_user.id != user.id {
                return Err(DomainError::UsernameAlreadyExists(user.username));
            }
        }

        // Check if new email conflicts with another user
        if let Some(existing_user) = self.user_repository.find_by_email(&user.email).await? {
            if existing_user.id != user.id {
                return Err(DomainError::EmailAlreadyExists(user.email));
            }
        }

        self.user_repository.update(&user).await
    }

    /// Delete user
    pub async fn delete_user(&self, id: i32) -> Result<(), DomainError> {
        // Check if user exists
        self.get_user_by_id(id).await?;
        
        self.user_repository.delete(id).await
    }

    /// Get all users
    pub async fn get_all_users(&self) -> Result<Vec<User>, DomainError> {
        self.user_repository.find_all().await
    }

    /// Find user by username
    pub async fn find_user_by_username(&self, username: &str) -> Result<Option<User>, DomainError> {
        self.user_repository.find_by_username(username).await
    }
}
