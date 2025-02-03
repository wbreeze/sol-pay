'use client';

import styles from "./page.module.css";
import { useState, useEffect } from 'react';
import QRCode from './components/QRCode';

export default function Home() {
  const [ routes, setRoutes ] = useState([]);
  const [ qrURL, setQRC ] = useState('');

  useEffect(() => {
    const ssrCatch =
      (typeof location === 'undefined') ? 'http://localhost' : location;
    async function fetchRoutes() {
      const url = new URL('/api/', ssrCatch);
      const data = await fetch(url);
      const fetched = await data.json();
      setRoutes(fetched);
    }
    fetchRoutes();
    setQRC("solana:" + new URL('api/tx/', ssrCatch));
  }, []);

  return (
    <div className={styles.page}>
      <main className={styles.main}>
        <QRCode qrData={qrURL} />
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

