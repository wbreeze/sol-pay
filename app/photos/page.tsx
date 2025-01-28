'use client';

import { useState } from "react";

import { RowsPhotoAlbum } from "react-photo-album";
import "react-photo-album/rows.css";
import "./page.css";

import Lightbox from "yet-another-react-lightbox";
import Fullscreen from "yet-another-react-lightbox/plugins/fullscreen";
import Download from "yet-another-react-lightbox/plugins/download";
import "yet-another-react-lightbox/styles.css";

import photos from "./photos";

export default function App() {
  const [index, setIndex] = useState(-1);

  function interceptDownload(indexo: { index: number }) {
    alert(`Download ${index}`);
  }

  return (
    <>
      <RowsPhotoAlbum photos={photos} targetRowHeight={150}
       spacing={4} padding={5}
       onClick={({ index }) => setIndex(index)} />

      <Lightbox
        slides={photos}
        open={index >= 0}
        index={index}
        close={() => setIndex(-1)}
        plugins={[Fullscreen, Download]}
        on={{ download: interceptDownload }}
      />
    </>
  );
}
