import { gql } from '@apollo/client';
import React, { useState, useEffect } from 'react';

const BASE_QUERY = gql`
  query BaseQuery($id: ID!) @catch {
    user(id: $id) {
      id
      name
      dynamicField @throwOnFieldError
    }
  }
`;

export default function DynamicComponent() {
  const [additionalQuery, setAdditionalQuery] = useState<any>(null);

  useEffect(() => {
    // Dynamic import with GraphQL - this should still be analyzed
    import('./additional-queries').then((module) => {
      setAdditionalQuery(module.ADDITIONAL_QUERY);
    });
  }, []);

  // GraphQL in template literal function call
  const createQuery = (fieldName: string) => gql`
    query DynamicQuery($id: ID!) {
      user(id: $id) {
        id
        ${fieldName} @throwOnFieldError
      }
    }
  `;

  return <div>Dynamic Component</div>;
}

export { BASE_QUERY };