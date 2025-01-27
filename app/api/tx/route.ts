import {
  address, createSolanaRpc,
  createTransactionMessage, setTransactionMessageFeePayer,
  setTransactionMessageLifetimeUsingBlockhash,
  appendTransactionMessageInstruction,
  compileTransaction, getBase64EncodedWireTransaction,
  lamports,
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

export async function POST(req: Request) {
  try {
    const DEVNET_RPC = "https://api.devnet.solana.com"
    const data = await req.json();
    console.log("POST Recieved", data);


    const payerAddressStr = data.account;
    const payerAddress = address(payerAddressStr);
    console.log("Payer address", payerAddress);

    const recipientAddressStr = process.env.TX_RECIPIENT_ACCOUNT;
    const recipientAddress = address(recipientAddressStr);
    console.log("Recipent address", recipientAddress);

    const lamportCount = lamports(BigInt(process.env.TX_LAMPORTS || 1));
    console.log("Lamports", lamportCount);

    const transfer = getTransferSolInstruction({
      source: payerAddress,
      destination: recipientAddress,
      amount: lamportCount,
    });
    console.log("Instruction", transfer);

    const rpcEndpoint = process.env.TX_RPC_ENDPOINT || DEVNET_RPC;
    const rpc = createSolanaRpc(rpcEndpoint);
    const { value: latestBlockhash } =
      await rpc.getLatestBlockhash({ commitment: 'confirmed' }).send();
    console.log("Latest hash", latestBlockhash);

    let txMsg = createTransactionMessage({ version: 0 });
    txMsg = setTransactionMessageFeePayer(recipientAddress, txMsg);
    txMsg = appendTransactionMessageInstruction(transfer, txMsg);
    txMsg = setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, txMsg);
    console.log("TransactionMessage", txMsg);

    const transaction = compileTransaction(txMsg as any);
    console.log("Compiled transaction");

    const base64Tx = getBase64EncodedWireTransaction(transaction);
    console.log("base64Tx:", base64Tx);
    const txPurpose =
      `Transfer ${lamportCount} lamports to ${recipientAddress}`;

    return Response.json({ transaction: base64Tx, message: txPurpose });
  } catch (error) {
    const text = `Error initiating transaction is ${error}`
    console.error(text);
    console.log(error.stack);
    const options = { status: 500, statusText: text }
    return new Response(text, options);
  }
}

