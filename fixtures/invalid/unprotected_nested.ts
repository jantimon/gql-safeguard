import { gql } from '@apollo/client';

const USER_BASIC_INFO = gql`
  fragment UserBasicInfo on User {
    id
    name
    email
  }
`;

const USER_AVATAR = gql`
  fragment UserAvatar on User {
    avatar @throwOnFieldError
    avatarUrl
  }
`;

const USER_DETAILS = gql`
  fragment UserDetails on User {
    ...UserBasicInfo
    ...UserAvatar
    bio
  }
`;

const GET_FULL_USER = gql`
  query GetFullUser($id: ID!) {
    user(id: $id) {
      ...UserDetails
    }
  }
`;

export { GET_FULL_USER, USER_DETAILS, USER_BASIC_INFO, USER_AVATAR };