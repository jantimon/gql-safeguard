import { gql } from 'relay';

// Basic @required(action: THROW) protected by query-level @catch
const GET_USER_BASIC = gql`
  query GetUserBasic($id: ID!) @catch {
    user(id: $id) {
      id
      name @required(action: THROW)
      email @required(action: THROW)
    }
  }
`;

// @required(action: THROW) protected by field-level @catch
const GET_USER_PROFILE = gql`
  query GetUserProfile($id: ID!) {
    user(id: $id) @catch {
      id
      name @required(action: THROW)
      avatar @required(action: THROW)
      bio
    }
  }
`;

// Mixed @required(action: THROW) and @throwOnFieldError with proper protection
const GET_USER_MIXED = gql`
  query GetUserMixed($id: ID!) @catch {
    user(id: $id) {
      id
      name @required(action: THROW)
      avatar @throwOnFieldError
      email @required(action: THROW)
    }
  }
`;

// Nested @required(action: THROW) with nested @catch protection
const GET_USER_NESTED = gql`
  query GetUserNested($id: ID!) {
    user(id: $id) {
      id
      profile @catch {
        displayName @required(action: THROW)
        bio @required(action: THROW)
        avatar {
          url @throwOnFieldError
        }
      }
    }
  }
`;

export default function UserComponent() {
  return "User Component";
}