'use client';

import QRCode from './QRCode';
import { useRef, createRef, useState } from 'react';
import "./POS.css";

export default function POS(props) {
  const account = props.baseURL || 'Set Account';
  const qrcBase = 'solana:' + account + '?'

  const DEFAULT_PESO = 100;
  const DEFAULT_CAMBIO = 8000;
  const DEFAULT_PRICE = milliSolDesdePeso(DEFAULT_PESO, DEFAULT_CAMBIO);

  const [qrURL, setQR] = useState(qrAmount(DEFAULT_PRICE));

  function qrAmount(milliSol) {
    return 'amount=' + (milliSol / 1_000).toFixed(7);
  }

  function memoValue(memo) {
    return (0 < memo.length) ? '&memo=' + encodeURIComponent(memo) : '';
  }

  function msgValue(msg) {
    return (0 < msg.length) ? '&message=' + encodeURIComponent(msg) : '';
  }

  function updateQRC(milliSol) {
      const eMemo = document.getElementById('memo');
      const memo = eMemo.value.trim();
      const msg = "Mi Empresa";
      const data = qrcBase + qrAmount(milliSol) +
        msgValue(msg) + memoValue(memo);
      setQR(data);
  }

  function pesoDesdeMilliSol(milliSol, dps) {
    return dps * milliSol / 1_000;
  }

  function milliSolDesdePeso(peso, dps) {
    return peso * 1_000 / dps;
  }

  function handleAmount(ev) {
    try {
      const amount = Number(ev.target.value);
      const eCambio = document.getElementById('cambio');
      const cambio = Number(eCambio.value);
      const ePeso = document.getElementById('peso');
      ePeso.value = pesoDesdeMilliSol(amount, cambio).toFixed(2);
      updateQRC(amount);
    } catch(e) {
      console.log(e, "Amount or cambio is not a number");
    }
  }

  function handleCambio(ev) {
    try {
      const cambio = Number(ev.target.value);
      const eAmount = document.getElementById('amount');
      const amount = Number(eAmount.value);
      const ePeso = document.getElementById('peso');
      ePeso.value = pesoDesdeMilliSol(amount, cambio).toFixed(2);
    } catch(e) {
      console.log(e, "Cambio o amount no es número");
    }
  }

  function handlePeso(ev) {
    try {
      const amount = Number(ev.target.value);
      const eCambio = document.getElementById('cambio');
      const cambio = Number(eCambio.value);
      const eAmount = document.getElementById('amount');
      eAmount.value = milliSolDesdePeso(amount, cambio).toFixed(4);
      updateQRC(eAmount.value);
    } catch(e) {
      console.log(e, "Pesos o cambio no es número");
    }
  }

  function handleMemo(ev) {
    try {
      const eAmount = document.getElementById('amount');
      updateQRC(eAmount.value);
    } catch(e) {
      console.log(e, "Amount is not a number");
    }
  }

  return (
    <>
      <div className='posQR'>
        <QRCode qrData={qrURL} />
      </div>
      <div className='pos-peso'>
        <label htmlFor='peso'>Pesos:</label>
        <input type='number' id='peso'
         defaultValue={DEFAULT_PESO} min='1' max='1000' step='20'
         onChange={handlePeso}
        />U$ pesos
      </div>
      <div className='pos-cambio'>
        <label htmlFor='cambio'>Cambio:</label>
        <input type='number' id='cambio'
         defaultValue={DEFAULT_CAMBIO} min='4000' max='16000' step='20'
         onChange={handleCambio}
        />U$ pesos por Sol
      </div>
      <div className='pos-amount'>
        <label htmlFor='amount'>Factura:</label>
        <input type='number' id='amount'
         defaultValue={DEFAULT_PRICE} min='1' max='3000' step='10'
         onChange={handleAmount}
        />/1000 Sol
      </div>
      <div className='pos-memo'>
        <label htmlFor='memo'>Referente:</label>
        <input type='text' id='memo' defaultValue=""
         onChange={handleMemo} />
      </div>
    </>
  );
}

