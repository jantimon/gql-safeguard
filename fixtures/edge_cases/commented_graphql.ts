import { gql } from 'relay';

// This should be ignored - it's in a comment
/*
const COMMENTED_QUERY = gql`
  query CommentedQuery {
    user {
      badField @throwOnFieldError
    }
  }
`;
*/

const VALID_QUERY = gql`
  query ValidQueryCommented($id: ID!) @catch {
    user(id: $id) {
      id
      name
      # This field comment should not affect parsing
      avatar @throwOnFieldError  # Protected by query-level @catch
    }
  }
`;

// Another commented GraphQL that should be ignored
// const IGNORED = gql`query BadQuery { user { field @throwOnFieldError } }`;

const STRING_WITH_GRAPHQL = "This string contains gql` query { user { field @throwOnFieldError } }` but should be ignored";

export { VALID_QUERY };