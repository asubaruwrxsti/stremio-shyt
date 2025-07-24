use crate::entities::User;
use crate::errors::DomainError;
use async_trait::async_trait;

/// Repository trait - defines what we need from persistence layer
/// This is a PORT in hexagonal architecture
#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_by_id(&self, id: i32) -> Result<Option<User>, DomainError>;
    async fn find_by_username(&self, username: &str) -> Result<Option<User>, DomainError>;
    async fn find_by_email(&self, email: &str) -> Result<Option<User>, DomainError>;
    async fn save(&self, user: &User) -> Result<User, DomainError>;
    async fn update(&self, user: &User) -> Result<User, DomainError>;
    async fn delete(&self, id: i32) -> Result<(), DomainError>;
    async fn find_all(&self) -> Result<Vec<User>, DomainError>;
}
