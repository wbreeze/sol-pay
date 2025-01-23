import {
  address, createSolanaRpc, getLatestBlockhash,
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

export async function POST(req: NextRequest) {
  try {
    const DEVNET_RPC = "https://api.devnet.solana.com"
    const data = await req.json();
    console.log("POST Recieved", data);

    const toAddressStr = process.env.TX_RECIPIENT_ACCOUNT;
    console.log("Transfer to", toAddressStr);

    const payerAddressStr = data.payerAccount;
    console.log("Payer account", payerAddressStr);

    const referenceStr = data.reference;
    console.log("Reference address", referenceStr);

    const payerAddress = address(payerAddressStr);
    const recipientAddress = address(toAddressStr);
    const reference = address(referenceStr);

    const lamportCount = Number(process.env.TX_LAMPORTS) || 1;
    console.log("Lamports", lamportCount);

    const payerAccount = payerAddress; //new Account(payerAddress);
    const recipientAccount = recipientAddress; //new Account(recipentAddress);

    const instruction = getTransferSolInstruction({
      source: payerAccount,
      destination: recipientAccount,
      amount: lamports(lamportCount),
    });
    console.log("Instruction");

    const rpcEndpoint = process.env.TX_RPC_ENDPOINT || DEVNET_RPC;
    const rpc = createSolanaRpc(rpcEndpoint);
    const { value: latestBlockhash } =
      await rpc.getLatestBlockhash({ commitment: 'confirmed' }).send();
    console.log("Latest hash", latestBlockhash);

    let tx = createTransactionMessage({ version: 0 });
    tx = setTransactionMessageFeePayer(payerAccount, tx);
    tx = appendTransactionMessageInstruction(instruction, tx);
    tx = setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, tx);
    console.log("TransactionMessage");

    const transaction = compileTransaction(tx);
    console.log("Compiled transaction");

    const base64Tx = getBase64EncodedWireTransaction(transaction);
    console.log("base64Tx:", base64Tx);
    const txPurpose =
      `Transfer ${lamportCount} lamports to ${recipientAccount}`;

    return Response.json({ transaction: base64Tx, message: txPurpose });
  } catch (error) {
    const text = `Error initiating transaction is ${error}`
    console.error(text);
    console.log(error.stack);
    const options = { status: 500, statusText: text }
    return new Response(text, options);
  }
}

