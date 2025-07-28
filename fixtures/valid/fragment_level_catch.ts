import { gql } from 'relay';

const RISKY_USER_DATA = gql`
  fragment RiskyUserData on User @catch {
    sensitiveInfo @throwOnFieldError
    backupEmail @throwOnFieldError
    internalId
  }
`;

const GET_USER_WITH_RISKY_DATA = gql`
  query GetUserWithRiskyData($id: ID!) @catch{
    user(id: $id) {
      id
      name
      friends @throwOnFieldError {
        ...RiskyUserData
      }
    }
  }
`;

export { GET_USER_WITH_RISKY_DATA, RISKY_USER_DATA };