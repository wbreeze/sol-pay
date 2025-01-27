'use client';

import styles from "./page.module.css";
import { useState, useEffect } from 'react';
import QRCode from './components/QRCode';

export default function Home() {
  const [ routes, setRoutes ] = useState([]);
  const [ qrURL, setQRC ] = useState('');

  useEffect(() => {
    async function fetchRoutes() {
      const url = new URL('/api/', location);
      const data = await fetch(url);
      const fetched = await data.json();
      setRoutes(fetched);
    }
    fetchRoutes();
    setQRC("solana:" + location + 'api/tx/');
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

