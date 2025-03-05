import './page.css';
import POS from '../components/POS';

export default async function Page({ searchParams }) {
  const query = await searchParams;

  return (
    <POS baseURL={query.account}/>
  );
}

