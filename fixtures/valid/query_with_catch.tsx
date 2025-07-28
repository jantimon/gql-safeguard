import { gql } from '@apollo/client';

const GET_USER_PROFILE = gql`
  query GetUserProfile($id: ID!) @catch {
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