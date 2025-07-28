import { gql } from 'relay';

const GET_USER_PROFILE_UNPROTECTED = gql`
  query GetUserProfileUnprotected($id: ID!) {
    user(id: $id) {
      id
      name
      avatar @throwOnFieldError
      email
    }
  }
`;

export default function UserProfile({ userId }: { userId: string }) {
  return <div>User Profile Component</div>;
}