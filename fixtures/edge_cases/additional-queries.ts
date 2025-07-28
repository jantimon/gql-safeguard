import { gql } from 'relay';

export const ADDITIONAL_QUERY = gql`
  query AdditionalQueryEdgeCase($id: ID!) {
    user(id: $id) {
      id
      importedField @throwOnFieldError  # Some Comment
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