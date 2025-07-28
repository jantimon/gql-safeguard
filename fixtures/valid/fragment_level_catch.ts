import { gql } from '@apollo/client';

const RISKY_USER_DATA = gql`
  fragment RiskyUserData on User @catch {
    sensitiveInfo @throwOnFieldError
    backupEmail @throwOnFieldError
    internalId
  }
`;

const GET_USER_WITH_RISKY_DATA = gql`
  query GetUserWithRiskyData($id: ID!) {
    user(id: $id) {
      id
      name
      ...RiskyUserData
    }
  }
`;

export { GET_USER_WITH_RISKY_DATA, RISKY_USER_DATA };