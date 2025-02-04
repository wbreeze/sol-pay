'use client';

import { useEffect, useState } from 'react';
//import "./API.css";

export default function API(props) {
  const [ routes, setRoutes ] = useState([]);
  const [ account, setAccount ] = useState('Set account');

  useEffect(() => {
    async function fetchRoutes() {
      const data = await fetch('/api');
      const fetched = await data.json();
      setRoutes(fetched.api);
      setAccount(fetched.account);
    }
    fetchRoutes();
  }, []);

  function mapRoutes() {
    return routes.map((route, index) => {
      let hrefURL = route.path;
      if (route.path === '/pos') {
        hrefURL = '/pos?account=' + account;
      }
      return (
        { urlRef: hrefURL,
          index: index,
          path: route.path,
          methods: route.methods,
        }
      );
    });
  }

  return (
    <ul className='api-routes'>
      {mapRoutes().map(({urlRef, index, path, methods}) => (
        <li key={index}>
          {methods.join(", ").toUpperCase()}: <a
            href={urlRef}>{path}</a>
        </li>
      ))}
    </ul>
  );
}

