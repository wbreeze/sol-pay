import {
  //clusterApiUrl, Cluster, Connection, Keypair, PublicKey,
  //LAMPORTS_PER_SOL, SystemProgram, Transaction,
  Address
} from "@solana/web3.js"

export async function GET(req: Request) {
  try {
    return Response.json({
      label: process.env.TX_LABEL || "Solana Transaction",
      icon: "https://solana.com/src/img/branding/solanaLogoMark.svg",
    });
  } catch (error) {
    const text = 'Error identifying recipient is ' + error;
    console.error(text);
    const options = { status: 500, statusText: text }
    return new Response(text, options);
  }
}

export async function POST(req: Request) {
  try {
    const recipientAcct = "AcctPlaceholder";
//    const recipientAcct = process.env.TX_RECIPIENT_KEY ||
//      Keypair.generate().publicKey;
//    const networkId = (process.env.TX_NETWORK || 'devnet') as Cluster;
    const lamportCount = Number(process.env.TX_LAMPORTS) || 1;
//
//    const { payerAccount, reference } = req.body;
//    const payerAccountPK = new PublicKey(payerAccount);
//    const referencePK = new PublicKey(reference);
//    const connection = new Connection(clusterApiUrl(networkId));
//    const { blockhash } = await connection.getLatestBlockhash();
//
//    const transaction = new Transaction({
//      recentBlockhash: blockhash,
//      feePayer: payerAccountPK,
//    });
//
//    const instruction = SystemProgram.transfer({
//      fromPubkey: new PublicKey(payerAccount),
//      toPubkey: new PublicKey(recipientAcct),
//      lamports: lamportCount,
//    });
//
//    instruction.keys.push({
//      pubkey: referencePK,
//      isSigner: false,
//      isWritable: false,
//    });
//
//    transaction.add(instruction);
//
//    const serializedTransaction = transaction.serialize({
//      requireAllSignatures: false,
//    });
//
//    const base64TxAddr = serializedTransaction.toString("base64");
    const txPurpose =
      `Transfer ${lamportCount} lamports to ${recipientAcct}`;

    return Response.json({
//      transaction: base64TxAddr,
      message: txPurpose
    });
  } catch (error) {
    const text = 'Error initiating transaction is ' + error;
    console.error(text);
    const options = { status: 500, statusText: text }
    return new Response(text, options);
  }
}

