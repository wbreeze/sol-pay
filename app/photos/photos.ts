import type { Photo } from "react-photo-album";

const breakpoints = [
  {size:400, subdir:'thumbnail'},
  {size:1600, subdir:'preview'},
];

function assetLink(dir, subdir, photo) {
  const root = 'https://wbreeze.com/photo/gallery';
  let link;
  if (subdir) {
    link = `${root}/${dir}/${subdir}/${photo}`;
  } else {
    link = `${root}/${dir}/${photo}`;
  }
  return link;
}

const photos = [
  {
    dir: "20240620GC/Day01r",
    photo: "DCL_0211.JPG",
    width: 1600,
    height: 1068,
    alt: "Navajo Bridges",
  },
  {
    dir: "20240620GC/Day01r",
    photo: "DSCF0012.JPG",
    width: 1600,
    height: 1200,
    alt: "Day One Rapids",
  },
  {
    dir: "20240620GC/Day02",
    photo: "DCL_0403.JPG",
    width: 1068,
    height: 1600,
    alt: "Day Two Canyon",
  },
  {
    dir: "20240620GC/Day02",
    photo: "DSCF0092.JPG",
    width: 1600,
    height: 1200,
    alt: "Day Two Rapids",
  },
  {
    dir: "20240620GC/Day03",
    photo: "DCL_0571.JPG",
    width: 1600,
    height: 1068,
    alt: "From inside cavern",
  },
  {
    dir: "20240620GC/Day02",
    photo: "DCL_0394.JPG",
    width: 1600,
    height: 1068,
    alt: "River against cliff side",
  },
  {
    dir: "20240620GC/Day03",
    photo: "DCL_0663.JPG",
    width: 1600,
    height: 1068,
    alt: "River bend pink cliffs",
  },
  {
    dir: "20240620GC/Day03",
    photo: "DCL_0738.JPG",
    width: 1600,
    height: 1068,
    alt: "Sedimentary cliffs overlooking river bend",
  },
  {
    dir: "20240620GC/Day04",
    photo: "DCL_0846.JPG",
    width: 1600,
    height: 1068,
    alt: "White granite cliffs",
  },
  {
    dir: "20240620GC/Day04",
    photo: "DCL_0928.JPG",
    width: 1068,
    height: 1600,
    alt: "Overlooking river valley",
  },
  {
    dir: "20240620GC/Day04",
    photo: "DCL_1003.JPG",
    width: 1600,
    height: 1068,
    alt: "Calcium carbonate blue Little Colorado River",
  },
  {
    dir: "20240620GC/Day04",
    photo: "DSC_1097.JPG",
    width: 1600,
    height: 1068,
    alt: "Red cliffs at sunset",
  },
  {
    dir: "20240620GC/Day05",
    photo: "DCL_1224.JPG",
    width: 1600,
    height: 1068,
    alt: "Riffle, green river bank, red sedimentary hills"
  },
  {
    dir: "20240620GC/Day06",
    photo: "DCL_1658.JPG",
    width: 1600,
    height: 1068,
    alt: "Jubilation after running rapid"
  },
  {
    dir: "20240620GC/Day06",
    photo: "DCL_1822.JPG",
    width: 1600,
    height: 1068,
    alt: "Scree slopes, granite cliffs, river"
  },
  {
    dir: "20240620GC/Day07",
    photo: "DCL_1857.JPG",
    width: 1600,
    height: 1068,
    alt: "Rapids"
  },
  {
    dir: "20240620GC/Day07/curl",
    photo: "DCL_1875.JPG",
    width: 1600,
    height: 1068,
    alt: "Curl in rapids"
  },
  {
    dir: "20240620GC/Day07",
    photo: "DCL_1989.JPG",
    width: 1600,
    height: 1068,
    alt: "River running through volcanic schist"
  },
  {
    dir: "20240620GC/Day08",
    photo: "DCL_2103.JPG",
    width: 1600,
    height: 1068,
    alt: "River running through volcanic schist"
  },
  {
    dir: "20240620GC/Day08",
    photo: "DCL_2233.JPG",
    width: 1068,
    height: 1600,
    alt: "Waterfall"
  },
  {
    dir: "20240620GC/Day09",
    photo: "DCL_2303.JPG",
    width: 1600,
    height: 1068,
    alt: "Raft in brown river"
  },
  {
    dir: "20240620GC/Day09",
    photo: "DSC_2368.JPG",
    width: 1600,
    height: 1068,
    alt: "Two solo boats alone in the canyon"
  },
  {
    dir: "20240620GC/Day10",
    photo: "DCL_2517.JPG",
    width: 1600,
    height: 1068,
    alt: "Waterfall"
  },
  {
    dir: "20240620GC/Day10",
    photo: "DCL_2600.JPG",
    width: 1600,
    height: 1068,
    alt: "Green valley"
  },
  {
    dir: "20240620GC/Day10",
    photo: "DCL_2715.JPG",
    width: 1068,
    height: 1600,
    alt: "Bottom of fall"
  },
  {
    dir: "20240620GC/Day10",
    photo: "DSC_2703.JPG",
    width: 1600,
    height: 1068,
    alt: "Brown river below striated cliffs"
  },
  {
    dir: "20240620GC/Day11",
    photo: "DCL_2743.JPG",
    width: 1600,
    height: 1068,
    alt: "Two rafts on the river in the morning"
  },
  {
    dir: "20240620GC/Day11",
    photo: "DCL_2807.JPG",
    width: 1600,
    height: 1068,
    alt: "Two rafts in rapids"
  },
  {
    dir: "20240620GC/Day12",
    photo: "DCL_3044.JPG",
    width: 1068,
    height: 1600,
    alt: "River polished canyon"
  },
  {
    dir: "20240620GC/Day12",
    photo: "DSC_3065.JPG",
    width: 1600,
    height: 1068,
    alt: "Pink cliffs at sunset"
  },
  {
    dir: "20240620GC/Day13",
    photo: "DCL_3098.jpeg",
    width: 1600,
    height: 1068,
    alt: "Deep blue sky over canyon walls"
  },
  {
    dir: "20240620GC/Day13/outtakes",
    photo: "DCL_3251.JPG",
    width: 1600,
    height: 1068,
    alt: "Inflatable kayak in Lava rapids"
  },
  {
    dir: "20240620GC/Day14",
    photo: "DCL_3509.JPG",
    width: 1068,
    height: 1600,
    alt: "Cactus"
  },
  {
    dir: "20240620GC/Day15",
    photo: "DCL_3714.JPG",
    width: 1600,
    height: 1068,
    alt: "Brown river bend in lower canyon"
  },
  {
    dir: "20240620GC/Day16",
    photo: "DCL_3760.JPG",
    width: 1600,
    height: 1068,
    alt: "Vertical cliffs at morning"
  },
  {
    dir: "20240620GC/Day16",
    photo: "DCL_3850.JPG",
    width: 1600,
    height: 1068,
    alt: "Brown river running through hills"
  },
].map(
  ({ dir, photo, alt, width, height }) =>
    ({
      src: assetLink(dir, null, photo),
      alt,
      width,
      height,
      srcSet: breakpoints.map((bp) => ({
        src: assetLink(dir, bp.subdir, photo),
        width: bp.size,
        height: Math.round((height / width) * bp.size),
      })),
    }) as Photo,
);

export default photos;
