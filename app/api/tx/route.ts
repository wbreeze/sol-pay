import {
  Address, address, generateKeyPair, getAddressFromPublicKey,
  createSolanaRpc, getLatestBlockhash,
  createTransactionMessage, setTransactionMessageFeePayer,
  setTransactionMessageLifetimeUsingBlockhash,
  appendTransactionMessageInstructions,
  getBase64EncodedWireTransaction,
  getTransactionEncoder
} from "@solana/web3.js";
import { getTransferSolInstruction } from '@solana-program/system';

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

export async function POST(req: NextRequest) {
  try {
    const DEVNET_RPC = "https://api.devnet.solana.com"

    const data = await req.json();
    console.log("POST Recieved", data);

    const toAccount = process.env.TX_RECIPIENT_ACCOUNT;
    console.log("Transfer to", toAccount);

    const payerAccountStr = data.payerAccount;
    console.log("Payer occount", payerAccountStr);
    const referenceStr = data.reference;
    console.log("Reference address", referenceStr);

    const recipientAccount = address(toAccount);
    const payerAccount = address(payerAccountStr);
    const reference = address(referenceStr);

    const lamportCount = Number(process.env.TX_LAMPORTS) || 1;
    console.log("Lamports", lamportCount);

    const instruction = getTransferSolInstruction({
      source: payerAccount,
      destination: recipientAccount,
      amount: lamportCount
    });

    const rpcEndpoint = process.env.TX_RPC_ENDPOINT || DEVNET_RPC;
    const rpc = createSolanaRpc(rpcEndpoint);
    const latestBlock = await rpc.getLatestBlockhash().send();
    const latestBlockhash = latestBlock.value.blockhash;
    console.log("Latest hash", latestBlockhash);

    let tx = createTransactionMessage({ version: 0 });
    tx = setTransactionMessageFeePayer(payerAccount, tx);
    tx = setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, tx);
    tx = appendTransactionMessageInstructions([instruction], tx);

    console.log("Transaction", JSON.stringify(tx, null, 2));

    const tec = getTransactionEncoder();
    console.log("Encoder", tec);
    const serializedTransaction = tec.encode(tx);
    console.log("STx", serializedTransaction);

    //const base64TxAddr = getBase64EncodedWireTransaction(tx);
    const txPurpose =
      `Transfer ${lamportCount} lamports to ${recipientAccount}`;

    return Response.json({
      transaction: base64TxAddr,
      message: txPurpose
    });
  } catch (error) {
    const text = 'Error initiating transaction is ' + error;
    console.error(text);
    const options = { status: 500, statusText: text }
    return new Response(text, options);
  }
}

