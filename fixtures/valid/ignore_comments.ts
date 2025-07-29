import { gql } from 'relay';

// Test ignore functionality for @throwOnFieldError and @required directives
const GET_USER_WITH_IGNORES = gql`
  query GetUserWithIgnores($id: ID!) @catch {
    user(id: $id) {
      id
      name @throwOnFieldError  # Protected by query-level @catch
      
      # gql-safeguard-ignore
      email @throwOnFieldError  # Should be ignored by gql-safeguard
      
      profile {
        # gql-safeguard-ignore
        avatar @required(action: THROW)  # Should be ignored by gql-safeguard
        bio @required(action: THROW)     # Should still be validated (protected by query @catch)
      }
    }
  }
`;

// Test ignore with field-level @catch protection
const GET_USER_FIELD_CATCH = gql`
  query GetUserFieldCatch($id: ID!) {
    user(id: $id) @catch {
      id
      # gql-safeguard-ignore
      name @throwOnFieldError  # Should be ignored
      email @throwOnFieldError # Protected by field-level @catch
    }
    
    otherUser: user(id: "other") {
      # gql-safeguard-ignore
      avatar @required(action: THROW)  # Should be ignored (would normally fail)
    }
  }
`;

// Test mixed scenarios
const GET_USER_MIXED = gql`
  query GetUserMixed($id: ID!) {
    protectedUser: user(id: $id) @catch {
      # gql-safeguard-ignore
      badField @throwOnFieldError  # Ignored
      goodField @throwOnFieldError # Protected
    }
    
    unprotectedUser: user(id: "other") {
      # gql-safeguard-ignore
      ignoredField @required(action: THROW)  # Ignored (would normally fail)
    }
  }
`;

export { GET_USER_WITH_IGNORES, GET_USER_FIELD_CATCH, GET_USER_MIXED };