import styles from "./page.module.css";
import QRCode from './components/QRCode';
import API from './components/API';

export default function Home() {
  const qrURL = 'solana:/api/tx/';

  return (
    <div className={styles.page}>
      <main className={styles.main}>
        <QRCode qrData={qrURL} />
        <API />
      </main>
    </div>
  );
}

