'use client';

import { useState, useRef } from "react";

import { RowsPhotoAlbum } from "react-photo-album";
import "react-photo-album/rows.css";
import "./page.css";

import Lightbox from "yet-another-react-lightbox";
import Fullscreen from "yet-another-react-lightbox/plugins/fullscreen";
import Download from "yet-another-react-lightbox/plugins/download";
import "yet-another-react-lightbox/styles.css";

import Tip from "../components/Tip";

import photos from "./photos";

export default function App() {
  const [index, setIndex] = useState(-1);
  const [tipDisplayed, setTipDisplayed] = useState(false);
  const lightboxRef = useRef(null);

  function onDownload() {
    setIndex(-1);
    setTipDisplayed(true);
  }

  return (
    <>
      <RowsPhotoAlbum
       photos={photos}
       targetRowHeight={150}
       spacing={4} padding={5}
       onClick={({ index }) => setIndex(index)}
      />

      <Lightbox
        ref={lightboxRef}
        slides={photos}
        open={index >= 0}
        index={index}
        close={() => setIndex(-1)}
        plugins={[Fullscreen, Download]}
        on={{ download: onDownload }}
      />

      <Tip
        tipURL={'api/tx'}
        isOpen={tipDisplayed}
        onClose={() => setTipDisplayed(false)}
      />
    </>
  );
}
