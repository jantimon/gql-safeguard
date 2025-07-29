import { gql } from 'relay';

// Should ignore @required with action: LOG (not THROW)
const GET_USER_LOG_ACTION = gql`
  query GetUserLogAction($id: ID!) {
    user(id: $id) {
      id
      name @required(action: LOG)    // Should be ignored - no @catch needed
      email @required(action: THROW) @catch  // Should validate - needs protection
    }
  }
`;

// Should ignore @required without action argument
const GET_USER_NO_ACTION = gql`
  query GetUserNoAction($id: ID!) {
    user(id: $id) {
      id
      name @required                 // Should be ignored - no action specified
      email @required(action: THROW) @catch  // Should validate
    }
  }
`;

// Should ignore @required with other action values
const GET_USER_OTHER_ACTIONS = gql`
  query GetUserOtherActions($id: ID!) {
    user(id: $id) {
      id
      name @required(action: WARN)   // Should be ignored
      email @required(action: NONE)  // Should be ignored
      bio @required(action: THROW) @catch    // Should validate
    }
  }
`;

// Complex nested case with mixed directive types
const GET_USER_COMPLEX = gql`
  query GetUserComplex($id: ID!) @catch {
    user(id: $id) {
      id
      profile {
        name @required(action: LOG)    // Should be ignored
        displayName @required(action: THROW)  // Should validate (protected by query @catch)
        avatar @throwOnFieldError      // Should validate (protected by query @catch)
        bio @catch {
          text @required(action: THROW)  // Should validate (protected by bio @catch)
          lastModified @required(action: WARN)  // Should be ignored
        }
      }
    }
  }
`;

// Fragment with mixed @required variants
const USER_FRAGMENT = gql`
  fragment UserInfo on User {
    name @required(action: LOG)      // Should be ignored
    email @required(action: THROW)   // Should validate when fragment is used
    avatar @throwOnFieldError        // Should validate when fragment is used
  }
`;

const GET_USER_WITH_FRAGMENT = gql`
  query GetUserWithFragment($id: ID!) {
    user(id: $id) @catch {
      id
      ...UserInfo  // Fragment spreads should inherit protection
    }
  }
`;

export default function EdgeCaseComponent() {
  return "Edge Case Component";
}