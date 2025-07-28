import { gql } from 'relay';

const USER_BASIC_INFO_UNPROTECTED = gql`
  fragment UserBasicInfoUnprotected on User {
    id
    name
    email
  }
`;

const USER_AVATAR_UNPROTECTED = gql`
  fragment UserAvatarUnprotected on User @throwOnFieldError {
    avatar
    avatarUrl
  }
`;

const USER_DETAILS_UNPROTECTED = gql`
  fragment UserDetailsUnprotected on User {
    ...UserBasicInfoUnprotected
    ...UserAvatarUnprotected
    bio
  }
`;

const GET_FULL_USER_UNPROTECTED = gql`
  query GetFullUserUnprotected($id: ID!) {
    user(id: $id) {
      ...UserDetailsUnprotected
    }
  }
`;

export { GET_FULL_USER_UNPROTECTED, USER_DETAILS_UNPROTECTED, USER_BASIC_INFO_UNPROTECTED, USER_AVATAR_UNPROTECTED };