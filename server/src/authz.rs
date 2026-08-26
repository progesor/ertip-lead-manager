use thiserror::Error;

use crate::auth::AuthUser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Admin,
    Manager,
    Sales,
}

impl Role {
    pub fn parse(value: &str) -> Result<Self, AuthorizationError> {
        match value {
            "ADMIN" => Ok(Self::Admin),
            "MANAGER" => Ok(Self::Manager),
            "SALES" => Ok(Self::Sales),
            other => Err(AuthorizationError::InvalidRole(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    PersonnelRead,
    PersonnelManage,
    LeadAssign,
    LeadRead,
    LeadStatusUpdate,
    LeadContentUpdate,
    ImportManage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    pub user_id: String,
    pub role: Role,
}

impl Actor {
    pub fn from_auth_user(user: &AuthUser) -> Result<Self, AuthorizationError> {
        Ok(Self {
            user_id: user.id.clone(),
            role: Role::parse(&user.role)?,
        })
    }

    pub fn require(&self, action: Action) -> Result<(), AuthorizationError> {
        if is_allowed(self.role, action) {
            Ok(())
        } else {
            Err(AuthorizationError::Forbidden)
        }
    }

    pub fn lead_scope(&self) -> LeadScope<'_> {
        match self.role {
            Role::Admin | Role::Manager => LeadScope::All,
            Role::Sales => LeadScope::AssignedTo(&self.user_id),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeadScope<'a> {
    All,
    AssignedTo(&'a str),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthorizationError {
    #[error("action is forbidden for current user")]
    Forbidden,
    #[error("unsupported persisted role {0}")]
    InvalidRole(String),
}

pub fn is_allowed(role: Role, action: Action) -> bool {
    match action {
        Action::PersonnelRead => matches!(role, Role::Admin | Role::Manager),
        Action::PersonnelManage => matches!(role, Role::Admin),
        Action::LeadAssign | Action::ImportManage => matches!(role, Role::Admin | Role::Manager),
        Action::LeadRead | Action::LeadStatusUpdate | Action::LeadContentUpdate => {
            matches!(role, Role::Admin | Role::Manager | Role::Sales)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, Actor, AuthorizationError, LeadScope, Role, is_allowed};

    #[test]
    fn role_policy_matches_m6_contract() {
        assert!(is_allowed(Role::Admin, Action::PersonnelManage));
        assert!(is_allowed(Role::Admin, Action::LeadAssign));
        assert!(is_allowed(Role::Admin, Action::LeadContentUpdate));
        assert!(is_allowed(Role::Admin, Action::ImportManage));

        assert!(is_allowed(Role::Manager, Action::PersonnelRead));
        assert!(!is_allowed(Role::Manager, Action::PersonnelManage));
        assert!(is_allowed(Role::Manager, Action::LeadAssign));
        assert!(is_allowed(Role::Manager, Action::LeadContentUpdate));
        assert!(is_allowed(Role::Manager, Action::ImportManage));

        assert!(!is_allowed(Role::Sales, Action::PersonnelRead));
        assert!(!is_allowed(Role::Sales, Action::LeadAssign));
        assert!(is_allowed(Role::Sales, Action::LeadRead));
        assert!(is_allowed(Role::Sales, Action::LeadStatusUpdate));
        assert!(is_allowed(Role::Sales, Action::LeadContentUpdate));
        assert!(!is_allowed(Role::Sales, Action::ImportManage));
    }

    #[test]
    fn sales_scope_is_always_own_assignment() {
        let actor = Actor {
            user_id: "sales-1".to_string(),
            role: Role::Sales,
        };
        assert_eq!(actor.lead_scope(), LeadScope::AssignedTo("sales-1"));

        let manager = Actor {
            user_id: "manager-1".to_string(),
            role: Role::Manager,
        };
        assert_eq!(manager.lead_scope(), LeadScope::All);
    }

    #[test]
    fn unknown_persisted_role_fails_closed() {
        assert_eq!(
            Role::parse("ROOT"),
            Err(AuthorizationError::InvalidRole("ROOT".to_string()))
        );
    }
}
