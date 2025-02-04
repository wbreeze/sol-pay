'use client';

import QRCodeStyling from 'qr-code-styling';
import { createRef, useEffect } from 'react';

export default function QRCode(props) {
  const qrData = props.qrData || 'https://localhost:3000/';
  const qrSize = props.qrSize || 300;
  const qrId = props.qrId || 'qr-code';
  const showData = props.showData || true;
  const dataIsURL = props.dataIsURL || true;

  /*
   qrOptions are generated at https://qr-code-styling.com/
   exported and filtered through the `jq` pretty printer.
   These use the Solana logomark in the center.
   See https://solana.com/branding
  */
  const qrOptions = function (boxSize, data) {
    return({
      "type": "canvas",
      "shape": "square",
      "width": boxSize,
      "height": boxSize,
      "data": data,
      "margin": 10,
      "qrOptions": {
        "typeNumber": "0",
        "mode": "Byte",
        "errorCorrectionLevel": "Q"
      },
      "imageOptions": {
        "saveAsBlob": true,
        "hideBackgroundDots": true,
        "imageSize": 0.4,
        "margin": 4
      },
      "dotsOptions": {
        "type": "extra-rounded",
        "color": "#8e4af6",
        "roundSize": true,
        "gradient": {
          "type": "linear",
          "rotation": 0,
          "colorStops": [
            {
              "offset": 0,
              "color": "#2e7fcd"
            },
            {
              "offset": 1,
              "color": "#46aecb"
            }
          ]
        }
      },
      "backgroundOptions": {
        "round": 0,
        "color": "#ffffff"
      },
      "image": "data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMTAxIiBoZWlnaHQ9Ijg4IiB2aWV3Qm94PSIwIDAgMTAxIDg4IiBmaWxsPSJub25lIiB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciPgo8cGF0aCBkPSJNMTAwLjQ4IDY5LjM4MTdMODMuODA2OCA4Ni44MDE1QzgzLjQ0NDQgODcuMTc5OSA4My4wMDU4IDg3LjQ4MTYgODIuNTE4NSA4Ny42ODc4QzgyLjAzMTIgODcuODk0IDgxLjUwNTUgODguMDAwMyA4MC45NzQzIDg4SDEuOTM1NjNDMS41NTg0OSA4OCAxLjE4OTU3IDg3Ljg5MjYgMC44NzQyMDIgODcuNjkxMkMwLjU1ODgyOSA4Ny40ODk3IDAuMzEwNzQgODcuMjAyOSAwLjE2MDQxNiA4Ni44NjU5QzAuMDEwMDkyMyA4Ni41MjkgLTAuMDM1OTE4MSA4Ni4xNTY2IDAuMDI4MDM4MiA4NS43OTQ1QzAuMDkxOTk0NCA4NS40MzI0IDAuMjYzMTMxIDg1LjA5NjQgMC41MjA0MjIgODQuODI3OEwxNy4yMDYxIDY3LjQwOEMxNy41Njc2IDY3LjAzMDYgMTguMDA0NyA2Ni43Mjk1IDE4LjQ5MDQgNjYuNTIzNEMxOC45NzYyIDY2LjMxNzIgMTkuNTAwMiA2Ni4yMTA0IDIwLjAzMDEgNjYuMjA5NUg5OS4wNjQ0Qzk5LjQ0MTUgNjYuMjA5NSA5OS44MTA0IDY2LjMxNjkgMTAwLjEyNiA2Ni41MTgzQzEwMC40NDEgNjYuNzE5OCAxMDAuNjg5IDY3LjAwNjcgMTAwLjg0IDY3LjM0MzZDMTAwLjk5IDY3LjY4MDYgMTAxLjAzNiA2OC4wNTI5IDEwMC45NzIgNjguNDE1QzEwMC45MDggNjguNzc3MSAxMDAuNzM3IDY5LjExMzEgMTAwLjQ4IDY5LjM4MTdaTTgzLjgwNjggMzQuMzAzMkM4My40NDQ0IDMzLjkyNDggODMuMDA1OCAzMy42MjMxIDgyLjUxODUgMzMuNDE2OUM4Mi4wMzEyIDMzLjIxMDggODEuNTA1NSAzMy4xMDQ1IDgwLjk3NDMgMzMuMTA0OEgxLjkzNTYzQzEuNTU4NDkgMzMuMTA0OCAxLjE4OTU3IDMzLjIxMjEgMC44NzQyMDIgMzMuNDEzNkMwLjU1ODgyOSAzMy42MTUxIDAuMzEwNzQgMzMuOTAxOSAwLjE2MDQxNiAzNC4yMzg4QzAuMDEwMDkyMyAzNC41NzU4IC0wLjAzNTkxODEgMzQuOTQ4MiAwLjAyODAzODIgMzUuMzEwM0MwLjA5MTk5NDQgMzUuNjcyMyAwLjI2MzEzMSAzNi4wMDgzIDAuNTIwNDIyIDM2LjI3N0wxNy4yMDYxIDUzLjY5NjhDMTcuNTY3NiA1NC4wNzQyIDE4LjAwNDcgNTQuMzc1MiAxOC40OTA0IDU0LjU4MTRDMTguOTc2MiA1NC43ODc1IDE5LjUwMDIgNTQuODk0NCAyMC4wMzAxIDU0Ljg5NTJIOTkuMDY0NEM5OS40NDE1IDU0Ljg5NTIgOTkuODEwNCA1NC43ODc5IDEwMC4xMjYgNTQuNTg2NEMxMDAuNDQxIDU0LjM4NDkgMTAwLjY4OSA1NC4wOTgxIDEwMC44NCA1My43NjEyQzEwMC45OSA1My40MjQyIDEwMS4wMzYgNTMuMDUxOCAxMDAuOTcyIDUyLjY4OTdDMTAwLjkwOCA1Mi4zMjc3IDEwMC43MzcgNTEuOTkxNyAxMDAuNDggNTEuNzIzTDgzLjgwNjggMzQuMzAzMlpNMS45MzU2MyAyMS43OTA1SDgwLjk3NDNDODEuNTA1NSAyMS43OTA3IDgyLjAzMTIgMjEuNjg0NSA4Mi41MTg1IDIxLjQ3ODNDODMuMDA1OCAyMS4yNzIxIDgzLjQ0NDQgMjAuOTcwNCA4My44MDY4IDIwLjU5MkwxMDAuNDggMy4xNzIxOUMxMDAuNzM3IDIuOTAzNTcgMTAwLjkwOCAyLjU2NzU4IDEwMC45NzIgMi4yMDU1QzEwMS4wMzYgMS44NDM0MiAxMDAuOTkgMS40NzEwMyAxMDAuODQgMS4xMzQwOEMxMDAuNjg5IDAuNzk3MTMgMTAwLjQ0MSAwLjUxMDI5NiAxMDAuMTI2IDAuMzA4ODIzQzk5LjgxMDQgMC4xMDczNDkgOTkuNDQxNSAxLjI0MDc0ZS0wNSA5OS4wNjQ0IDBMMjAuMDMwMSAwQzE5LjUwMDIgMC4wMDA4NzgzOTcgMTguOTc2MiAwLjEwNzY5OSAxOC40OTA0IDAuMzEzODQ4QzE4LjAwNDcgMC41MTk5OTggMTcuNTY3NiAwLjgyMTA4NyAxNy4yMDYxIDEuMTk4NDhMMC41MjQ3MjMgMTguNjE4M0MwLjI2NzY4MSAxOC44ODY2IDAuMDk2NjE5OCAxOS4yMjIzIDAuMDMyNTE4NSAxOS41ODM5Qy0wLjAzMTU4MjkgMTkuOTQ1NiAwLjAxNDA2MjQgMjAuMzE3NyAwLjE2Mzg1NiAyMC42NTQ1QzAuMzEzNjUgMjAuOTkxMyAwLjU2MTA4MSAyMS4yNzgxIDAuODc1ODA0IDIxLjQ3OTlDMS4xOTA1MyAyMS42ODE3IDEuNTU4ODYgMjEuNzg5NiAxLjkzNTYzIDIxLjc5MDVaIiBmaWxsPSJ1cmwoI3BhaW50MF9saW5lYXJfMTc0XzQ0MDMpIi8+CjxkZWZzPgo8bGluZWFyR3JhZGllbnQgaWQ9InBhaW50MF9saW5lYXJfMTc0XzQ0MDMiIHgxPSI4LjUyNTU4IiB5MT0iOTAuMDk3MyIgeDI9Ijg4Ljk5MzMiIHkyPSItMy4wMTYyMiIgZ3JhZGllbnRVbml0cz0idXNlclNwYWNlT25Vc2UiPgo8c3RvcCBvZmZzZXQ9IjAuMDgiIHN0b3AtY29sb3I9IiM5OTQ1RkYiLz4KPHN0b3Agb2Zmc2V0PSIwLjMiIHN0b3AtY29sb3I9IiM4NzUyRjMiLz4KPHN0b3Agb2Zmc2V0PSIwLjUiIHN0b3AtY29sb3I9IiM1NDk3RDUiLz4KPHN0b3Agb2Zmc2V0PSIwLjYiIHN0b3AtY29sb3I9IiM0M0I0Q0EiLz4KPHN0b3Agb2Zmc2V0PSIwLjcyIiBzdG9wLWNvbG9yPSIjMjhFMEI5Ii8+CjxzdG9wIG9mZnNldD0iMC45NyIgc3RvcC1jb2xvcj0iIzE5RkI5QiIvPgo8L2xpbmVhckdyYWRpZW50Pgo8L2RlZnM+Cjwvc3ZnPgo=",
      "dotsOptionsHelper": {
        "colorType": {
          "single": true,
          "gradient": false
        },
        "gradient": {
          "linear": true,
          "radial": false,
          "color1": "#6a1a4c",
          "color2": "#6a1a4c",
          "rotation": "0"
        }
      },
      "cornersSquareOptions": {
        "type": "extra-rounded",
        "color": "#8d4bf5"
      },
      "cornersSquareOptionsHelper": {
        "colorType": {
          "single": true,
          "gradient": false
        },
        "gradient": {
          "linear": true,
          "radial": false,
          "color1": "#000000",
          "color2": "#000000",
          "rotation": "0"
        }
      },
      "cornersDotOptions": {
        "type": "",
        "color": "#6983d7"
      },
      "cornersDotOptionsHelper": {
        "colorType": {
          "single": true,
          "gradient": false
        },
        "gradient": {
          "linear": true,
          "radial": false,
          "color1": "#000000",
          "color2": "#000000",
          "rotation": "0"
        }
      },
      "backgroundOptionsHelper": {
        "colorType": {
          "single": true,
          "gradient": false
        },
        "gradient": {
          "linear": true,
          "radial": false,
          "color1": "#ffffff",
          "color2": "#ffffff",
          "rotation": "0"
        }
      }
    });
  };

  const qrRef = createRef();

  useEffect(() => {
    const qrCode = new QRCodeStyling(qrOptions(qrSize, qrData));
    qrRef.current.innerHTML = "";
    qrCode.append(qrRef.current);
  });

  return (
    <div className='qr-code'>
      <div className='image' ref={qrRef} />
      <div className='data'>
        <a href={qrData}>{qrData}</a>
      </div>
    </div>
  )
}

