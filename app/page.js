'use client';

import styles from "./page.module.css";
import { useState, useEffect } from 'react';

export default function Home() {
  const url = new URL('/api/', location);
  const [ routes, setRoutes ] = useState([]);

  useEffect(() => {
    async function fetchRoutes() {
      const data = await fetch(url);
      const fetched = await data.json();
      setRoutes(fetched);
    }
    fetchRoutes();
  }, []);

  return (
    <div className={styles.page}>
      <main className={styles.main}>
        <ul className='api-routes'>
          {routes.map((route, index) => (
            <li key={index}>
              {route.methods.join(", ").toUpperCase()}: <a
                href={route.path}>{route.path}</a>
            </li>
          ))}
        </ul>
      </main>
    </div>
  );
}

