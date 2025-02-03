'use client';

/*
Thank you to Rahul in India
for the example code on GitHub
https://github.com/c99rahul/react-modal/
that displays and dismisses a dialog.

There is something in the Lightbox implementation
that aria- hides the close button when the lightbox
is showing. The button remains visible yet hidden from
any interaction.
*/

import QRCode from './QRCode';
import { useRef, createRef, useEffect } from 'react';
import "./Tip.css";

export default function Tip(props) {
  const tipURL = props.tipURL || '/';
  const isOpen = props.isOpen || false;
  const onClose = props.onClose || null;
  const tipRef = useRef(null);

  const ssrCatch = location || null;
  const tipQRData = 'solana:' + new URL(tipURL, ssrCatch);

  function handleCloseTip() {
    if (onClose) {
      onClose();
    }
  };

  useEffect(() => {
    const tipElement = tipRef.current;
    console.log("Have tip element:", tipElement);
    if (tipElement) {
      if (isOpen) {
        tipElement.showModal();
      } else {
        tipElement.close();
      }
    }
  }, [isOpen]);

  return (
    <dialog ref={tipRef} className="tip">
      <p className='tip-text-top'>Gracias por descargar.<br/>
        ¡Me gusta que te guste!</p>
      <QRCode qrData={tipQRData} />
      <p className='tip-text'>Escanea este código QR (o copia el dirección)
      con tu billetera Solana para dar una propina de
      0,001 SOL. Recuerda que la licencia es</p>
      <a href="https://creativecommons.org/licenses/by-nc-nd/4.0/">
        CC BY_NC_ND
      </a>
      <p className="tip-cc">Copyright Douglas Lovell.</p>
      <button
        onClick={handleCloseTip}
        className='tip-dismiss'
        aria-label='Cerrá pedido'
      >Cerrar</button>
    </dialog>
  )
}

