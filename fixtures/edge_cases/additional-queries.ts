import { gql } from '@apollo/client';

export const ADDITIONAL_QUERY = gql`
  query AdditionalQuery($id: ID!) {
    user(id: $id) {
      id
      importedField @throwOnFieldError  # Missing @catch - should be flagged
    }
  }
`;

export const GRAPHQL_QUERY = gql`
  query GraphQLQuery($id: ID!) @catch {
    user(id: $id) {
      id
      protectedField @throwOnFieldError
    }
  }
`;