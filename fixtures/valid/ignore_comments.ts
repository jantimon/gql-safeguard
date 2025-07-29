import { gql } from 'relay';

const GET_USER_WITH_IGNORES = gql`
  query GetUserFieldCatch($id: ID!) {
    user(id: $id) {
      id
      # gql-safeguard-ignore
      name @throwOnFieldError  # Should be ignored
    }
    
    otherUser: user(id: "other") {
      # gql-safeguard-ignore
      avatar @required(action: THROW)  # Should be ignored (would normally fail)
    }
  }
`;

const GET_USER_WITH_IGNORES_WITH_FRAGMENT = gql`
  query GetUserFieldIgnoreWithFragment($id: ID!) {
    user(id: $id) {
      ...userFieldsWithIgnore
    }
  }
`;

const userFields = gql`
  fragment userFieldsWithIgnore on User {
    id
    # gql-safeguard-ignore
    @throwOnFieldError # Should be ignored
    @argumentDefinitions(
        showPrivateData: { type: "Boolean!", defaultValue: false }
        includeMetadata: { type: "Boolean!", defaultValue: true }
        profileVersion: { type: "Int", defaultValue: 1 }
      ) {
      name   
    }
  }
`;

const userFieldsWithIgnoreInline = gql`
  # gql-safeguard-ignore
  fragment userFieldsWithIgnoreInline on User @throwOnFieldError{
    answers {
      ... on BlogPageCompetitionSelectionAnswer {
        id
        text
        participantCount
        selectedOption
        submissionDate
        contestId
      }
    }
  }
`;