import { gql } from 'relay';

const FRAGMENT_A = gql`
  fragment FragmentACircular on User {
    id
    name
    ...FragmentBCircular
  }
`;

const FRAGMENT_B = gql`
  fragment FragmentBCircular on User {
    email
    bio
    ...FragmentACircular  # Circular reference
  }
`;

const CIRCULAR_QUERY = gql`
  query CircularQueryTest($id: ID!) {
    user(id: $id) {
      ...FragmentACircular
    }
  }
`;

export { CIRCULAR_QUERY, FRAGMENT_A, FRAGMENT_B };