'use client';

import QRCode from './QRCode';
import { useRef, createRef, useState } from 'react';
//import "./POS.css";

export default function POS(props) {
  const account = props.baseURL || 'Set Account';
  const qrcBase = 'solana:' + account + '?'

  function qrAmount(milliSol) {
    const data = qrcBase + 'amount=' + milliSol / 1_000;
    return encodeURI(data);
  }

  function handleAmount(ev) {
    try {
      const amount = Number(ev.target.value);
      setQR(qrAmount(amount));
    } catch(e) {
      console.log(e, "Amount is not a number");
    }
  }

  const DEFAULT_PRICE = 20;
  const [qrURL, setQR] = useState(qrAmount(DEFAULT_PRICE));

  return (
    <>
      <div className='posQR'>
        <QRCode qrData={qrURL} />
      </div>
      <div className='pos-amount'>
        <label htmlFor='amount'>Factura</label>
        <input type='number' id='amount'
         defaultValue={DEFAULT_PRICE} min='1' max='3000' step='10'
         onChange={handleAmount}
        />/1000 Sol
      </div>
    </>
  );
}

