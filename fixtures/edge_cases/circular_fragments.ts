import { gql } from 'relay';

const FRAGMENT_A = gql`
  fragment FragmentA on User {
    id
    name
    ...FragmentB
  }
`;

const FRAGMENT_B = gql`
  fragment FragmentB on User {
    email
    bio
    ...FragmentA  # Circular reference
  }
`;

const CIRCULAR_QUERY = gql`
  query CircularQuery($id: ID!) {
    user(id: $id) {
      ...FragmentA
    }
  }
`;

export { CIRCULAR_QUERY, FRAGMENT_A, FRAGMENT_B };