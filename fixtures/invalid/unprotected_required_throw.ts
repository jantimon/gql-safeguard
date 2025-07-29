import { gql } from 'relay';

// ❌ Unprotected @required(action: THROW) - should fail validation
const GET_USER_UNPROTECTED = gql`
  query GetUserUnprotected($id: ID!) {
    user(id: $id) {
      id
      name @required(action: THROW)
      email
    }
  }
`;

// ❌ Partially protected - some fields unprotected
const GET_USER_PARTIAL = gql`
  query GetUserPartial($id: ID!) {
    user(id: $id) @catch {
      id
      name @required(action: THROW)  // ✅ Protected
      email
    }
    otherUser: user(id: "other") {
      name @required(action: THROW)  // ❌ Unprotected
    }
  }
`;

// ❌ Mixed unprotected directives
const GET_USER_MIXED_UNPROTECTED = gql`
  query GetUserMixedUnprotected($id: ID!) {
    user(id: $id) {
      id
      name @required(action: THROW)  // ❌ Unprotected
      avatar @throwOnFieldError      // ❌ Unprotected
    }
  }
`;

export default function UserComponent() {
  return "User Component";
}