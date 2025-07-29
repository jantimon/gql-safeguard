import { gql } from 'relay';

// Test case replicating query-level @throwOnFieldError with ignore comment
// This should fail validation because @throwOnFieldError at query level is unprotected
// The gql-safeguard-ignore comment only affects the @required(action: THROW) on the field
const QUERY_LEVEL_THROW_WITH_IGNORE = gql`
  query queryLevelThrowWithIgnore($groupId: ID!)
  @throwOnFieldError
  @raw_response_type {
    customerOrderProductLineItemsGroupById(id: $groupId)
      # gql-safeguard-ignore
      @required(action: THROW) {
      startWrongDeliveryRegistrationRelativeUrl
    }
  }
`;

// Additional test case: query-level @throwOnFieldError without any protection
const UNPROTECTED_QUERY_LEVEL_THROW = gql`
  query unprotectedQueryLevelThrow($id: ID!)
  @throwOnFieldError {
    user(id: $id) {
      id
      name
      email
    }
  }
`;

// Test case: query-level @throwOnFieldError with ignore comment on query itself
const QUERY_LEVEL_THROW_WITH_QUERY_IGNORE = gql`
  # gql-safeguard-ignore
  query queryLevelThrowWithQueryIgnore($id: ID!)
  @throwOnFieldError {
    user(id: $id) {
      id
      name
    }
  }
`;

export { QUERY_LEVEL_THROW_WITH_IGNORE, UNPROTECTED_QUERY_LEVEL_THROW, QUERY_LEVEL_THROW_WITH_QUERY_IGNORE };