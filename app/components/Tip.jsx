'use client';

import QRCode from './QRCode';
import { useState } from 'react';
import "./Tip.css";

export default function Tip(props) {
  const [ tip, setTip ] = useState('solana:https://');

  return (
    <div className='tip'>
      <p className='tip-text-top'>Gracias por descargar.<br/>
        ¡Me gusta que te guste!</p>
      <QRCode qrData={tip} />
      <p className='tip-text'>Escanea este código QR (o copia el dirección)
      con tu monedero Solana para dar una propina de
      0,001 SOL. Recuerda que la licencia es</p>
      <a href="https://creativecommons.org/licenses/by-nc-nd/4.0/">
        CC BY_NC_ND
      </a>
      <p className="tip-cc">Copyright Douglas Lovell.</p>
      <button className='tip-dismiss'>Dismiss</button>
    </div>
  )
}

