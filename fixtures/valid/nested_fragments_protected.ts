import { gql } from 'relay';

const USER_BASIC_INFO = gql`
  fragment UserBasicInfo on User {
    id
    name
    email
  }
`;

const USER_AVATAR_IMAGE = gql`
  fragment avatarImage on User @throwOnFieldError {
    avatar
    avatarUrl
  }
`;

const USER_AVATAR = gql`
  fragment UserAvatar on User @catch {
    avatar
    ...avatarImage
  }
`;

const USER_BIO = gql`
  fragment UserBio on User {
    bioText
    bioImage @throwOnFieldError
  }
`;

const USER_DETAILS = gql`
  fragment UserDetails on User {
    ...UserBasicInfo
    ...UserAvatar
    bio @catch(to: NULL) {
      ...UserBio
    }
  }
`;

const GET_FULL_USER = gql`
  query GetFullUser($id: ID!) {
    user(id: $id) {
      ...UserDetails
    }
  }
`;