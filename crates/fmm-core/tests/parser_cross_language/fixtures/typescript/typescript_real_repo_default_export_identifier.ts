
import { connect } from 'react-redux';
import { fetchUser } from './actions';

interface Props {
    userId: string;
    name: string;
}

function UserProfile({ userId, name }: Props) {
    return null;
}

const mapStateToProps = (state: any) => ({
    name: state.user.name,
});

const ConnectedProfile = connect(mapStateToProps)(UserProfile);

export default ConnectedProfile;
