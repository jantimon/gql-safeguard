import { gql } from 'relay';

const PROTECTED_FRAGMENT = gql`
  fragment ProtectedFragment on User @catch {
    sensitiveData @throwOnFieldError
    otherData
  }
`;

const UNPROTECTED_FRAGMENT = gql`
  fragment UnprotectedFragment on User {
    riskyField @throwOnFieldError
    normalField
  }
`;

const MIXED_QUERY = gql`
  query MixedQuery($id: ID!) {
    user(id: $id) {
      id
      ...ProtectedFragment
      ...UnprotectedFragment
    }
  }
`;

export { MIXED_QUERY, PROTECTED_FRAGMENT, UNPROTECTED_FRAGMENT };