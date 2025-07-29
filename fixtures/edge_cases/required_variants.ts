import { gql } from 'relay';

// Should ignore @required with action: LOG (not THROW)
const GET_USER_LOG_ACTION = gql`
  query GetUserLogAction($id: ID!) {
    user(id: $id) {
      id
      name @required(action: LOG)
      email @required(action: THROW) @catch
    }
  }
`;

// Should ignore @required without action argument
const GET_USER_NO_ACTION = gql`
  query GetUserNoAction($id: ID!) {
    user(id: $id) {
      id
      name @required
      email @required(action: THROW) @catch
    }
  }
`;

// Should ignore @required with other action values
const GET_USER_OTHER_ACTIONS = gql`
  query GetUserOtherActions($id: ID!) {
    user(id: $id) {
      id
      name @required(action: WARN)
      email @required(action: NONE)
      bio @required(action: THROW) @catch
    }
  }
`;

// Complex nested case with mixed directive types
const GET_USER_COMPLEX = gql`
  query GetUserComplex($id: ID!) @catch {
    user(id: $id) {
      id
      profile {
        name @required(action: LOG)
        displayName @required(action: THROW)
        avatar @throwOnFieldError
        bio @catch {
          text @required(action: THROW)
          lastModified @required(action: WARN)
        }
      }
    }
  }
`;

// Fragment with mixed @required variants
const USER_FRAGMENT = gql`
  fragment UserInfo on User {
    name @required(action: LOG)
    email @required(action: THROW)
    avatar @throwOnFieldError
  }
`;

const GET_USER_WITH_FRAGMENT = gql`
  query GetUserWithFragment($id: ID!) {
    user(id: $id) @catch {
      id
      ...UserInfo
    }
  }
`;

export default function EdgeCaseComponent() {
  return "Edge Case Component";
}